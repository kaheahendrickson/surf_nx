use std::env;
use std::error::Error as StdError;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use clap::{Args, Parser, Subcommand};
use serde::Deserialize;
use serde_json::{json, Value};
use solana_keypair::Keypair;
use solana_message::Message;
use solana_pubkey::Pubkey;
use solana_signer::{EncodableKey, Signer};
use solana_system_interface::instruction::transfer as sol_transfer_instruction;
use solana_transaction::Transaction;
use surf_client::{Backend, LocalKeypairSigner, ProgramAccountsFilter, QueryClient, Surf};
use surf_client_http_config::HttpBackendConfig;
use surf_client_backend_http::HttpBackend;

const ENV_TOKEN_PROGRAM_ID: &str = "SURF_TOKEN_PROGRAM_ID";
const ENV_REGISTRY_PROGRAM_ID: &str = "SURF_REGISTRY_PROGRAM_ID";
const ENV_SIGNALS_PROGRAM_ID: &str = "SURF_SIGNALS_PROGRAM_ID";
const ENV_RPC_URL: &str = "SURF_TEST_VALIDATOR_URL";
const ENV_KEYPAIR: &str = "SURF_KEYPAIR";
const DEFAULT_CONFIG_PATH: &str = "~/.config/surf-cli/config.json";
const DEFAULT_KEYPAIR: &str = "~/.config/solana/id.json";

#[derive(Parser, Debug)]
#[command(name = "surf-cli")]
#[command(about = "Native SURF CLI powered by surf-client and surf-client-backend-http")]
struct Cli {
    #[command(flatten)]
    global: GlobalArgs,
    #[command(subcommand)]
    command: Command,
}

#[derive(Args, Debug, Clone)]
struct GlobalArgs {
    #[arg(long, global = true)]
    rpc_url: Option<String>,
    #[arg(long, global = true)]
    config: Option<String>,
    #[arg(long, global = true)]
    token_program: Option<String>,
    #[arg(long, global = true)]
    registry_program: Option<String>,
    #[arg(long, global = true)]
    signals_program: Option<String>,
    #[arg(long, global = true)]
    json: bool,
}

#[derive(Args, Debug, Clone)]
struct KeypairArgs {
    #[arg(long)]
    keypair: Option<String>,
}

#[derive(Subcommand, Debug)]
enum Command {
    Config(ConfigCommand),
    Query(QueryCommand),
    Token(TokenCommand),
    Sol(SolCommand),
    Names(NamesCommand),
    Signals(SignalsCommand),
}

#[derive(Args, Debug)]
struct ConfigCommand {
    #[command(subcommand)]
    command: ConfigSubcommand,
}

#[derive(Subcommand, Debug)]
enum ConfigSubcommand {
    Show,
}

#[derive(Args, Debug)]
struct QueryCommand {
    #[command(subcommand)]
    command: QuerySubcommand,
}

#[derive(Subcommand, Debug)]
enum QuerySubcommand {
    TokenConfig,
    Balance { owner: String },
    RegistryConfig,
    SignalsConfig,
    NameRecord { name: String },
}

#[derive(Args, Debug)]
struct SignalsCommand {
    #[command(subcommand)]
    command: SignalsSubcommand,
}

#[derive(Args, Debug)]
struct SolCommand {
    #[command(subcommand)]
    command: SolSubcommand,
}

#[derive(Subcommand, Debug)]
enum SolSubcommand {
    Transfer {
        #[command(flatten)]
        keypair: KeypairArgs,
        #[arg(long)]
        recipient: String,
        #[arg(long)]
        lamports: u64,
    },
}

#[derive(Subcommand, Debug)]
enum SignalsSubcommand {
    Initialize {
        #[command(flatten)]
        keypair: KeypairArgs,
    },
    Follow {
        #[command(flatten)]
        keypair: KeypairArgs,
        #[arg(long)]
        target: String,
    },
    FollowName {
        #[command(flatten)]
        keypair: KeypairArgs,
        #[arg(long)]
        name: String,
    },
    Unfollow {
        #[command(flatten)]
        keypair: KeypairArgs,
        #[arg(long)]
        target: String,
    },
    UnfollowName {
        #[command(flatten)]
        keypair: KeypairArgs,
        #[arg(long)]
        name: String,
    },
}

#[derive(Args, Debug)]
struct TokenCommand {
    #[command(subcommand)]
    command: TokenSubcommand,
}

#[derive(Subcommand, Debug)]
enum TokenSubcommand {
    Initialize {
        #[command(flatten)]
        keypair: KeypairArgs,
        #[arg(long)]
        total_supply: u64,
        #[arg(long)]
        decimals: u8,
    },
    Mint {
        #[command(flatten)]
        keypair: KeypairArgs,
        #[arg(long)]
        recipient: String,
        #[arg(long)]
        amount: u64,
    },
    Transfer {
        #[command(flatten)]
        keypair: KeypairArgs,
        #[arg(long)]
        recipient: String,
        #[arg(long)]
        amount: u64,
    },
    Burn {
        #[command(flatten)]
        keypair: KeypairArgs,
        #[arg(long)]
        amount: u64,
    },
}

#[derive(Args, Debug)]
struct NamesCommand {
    #[command(subcommand)]
    command: NamesSubcommand,
}

#[derive(Subcommand, Debug)]
enum NamesSubcommand {
    Initialize {
        #[command(flatten)]
        keypair: KeypairArgs,
        #[arg(long)]
        price: u64,
    },
    Register {
        #[command(flatten)]
        keypair: KeypairArgs,
        #[arg(long)]
        name: String,
    },
    List,
    Lookup {
        name: String,
    },
}

#[derive(Debug)]
struct CliError(String);

impl fmt::Display for CliError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl StdError for CliError {}

type Result<T> = std::result::Result<T, Box<dyn StdError>>;

#[derive(Debug, Clone)]
struct AppConfig {
    config_path: PathBuf,
    rpc_url: String,
    token_program: Pubkey,
    registry_program: Pubkey,
    signals_program: Pubkey,
    default_keypair: PathBuf,
    json: bool,
}

impl AppConfig {
    fn from_args(args: &GlobalArgs) -> Result<Self> {
        let config_path = resolve_config_path(args.config.as_deref())?;
        let file_config =
            FileConfig::load(Some(config_path.as_os_str().to_string_lossy().as_ref()))?;
        let rpc_url = resolve_string_option(
            args.rpc_url.as_deref(),
            file_config.rpc_url.as_deref(),
            Some(ENV_RPC_URL),
            Some(HttpBackendConfig::from_env_or_default().url),
            "rpc url",
            Some("rpc-url"),
        )?;

        Ok(Self {
            config_path,
            rpc_url,
            token_program: resolve_program_id(
                args.token_program.as_deref(),
                file_config.token_program.as_deref(),
                Some(ENV_TOKEN_PROGRAM_ID),
                "token program id",
                "token-program",
            )?,
            registry_program: resolve_program_id(
                args.registry_program.as_deref(),
                file_config.registry_program.as_deref(),
                Some(ENV_REGISTRY_PROGRAM_ID),
                "registry program id",
                "registry-program",
            )?,
            signals_program: resolve_program_id(
                args.signals_program.as_deref(),
                file_config.signals_program.as_deref(),
                Some(ENV_SIGNALS_PROGRAM_ID),
                "signals program id",
                "signals-program",
            )?,
            default_keypair: resolve_keypair_path(
                None,
                file_config.keypair.as_deref(),
                Some(ENV_KEYPAIR),
                Some(DEFAULT_KEYPAIR.to_string()),
            )?,
            json: args.json,
        })
    }
}

#[derive(Debug, Default, Deserialize)]
struct FileConfig {
    rpc_url: Option<String>,
    token_program: Option<String>,
    registry_program: Option<String>,
    signals_program: Option<String>,
    keypair: Option<String>,
}

impl FileConfig {
    fn load(path: Option<&str>) -> Result<Self> {
        let path = resolve_config_path(path)?;
        if !path.exists() {
            return Ok(Self::default());
        }

        let contents = fs::read_to_string(&path).map_err(|err| {
            Box::new(CliError(format!(
                "failed to read config file {}: {err}",
                path.display()
            ))) as Box<dyn StdError>
        })?;

        serde_json::from_str(&contents).map_err(|err| {
            Box::new(CliError(format!(
                "failed to parse config file {}: {err}",
                path.display()
            ))) as Box<dyn StdError>
        })
    }
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    if let Err(err) = run(cli).await {
        eprintln!("Error: {err}");
        std::process::exit(1);
    }
}

async fn run(cli: Cli) -> Result<()> {
    let config = AppConfig::from_args(&cli.global)?;

    match cli.command {
        Command::Config(config_command) => run_config(&config, config_command),
        Command::Query(query) => run_query(&config, query).await,
        Command::Token(token) => run_token(&config, token).await,
        Command::Sol(sol) => run_sol(&config, sol).await,
        Command::Names(names) => run_names(&config, names).await,
        Command::Signals(signals) => run_signals(&config, signals).await,
    }
}

fn run_config(config: &AppConfig, command: ConfigCommand) -> Result<()> {
    match command.command {
        ConfigSubcommand::Show => {
            print_value(
                config.json,
                json!({
                    "config_path": config.config_path.display().to_string(),
                    "rpc_url": config.rpc_url,
                    "token_program": config.token_program.to_string(),
                    "registry_program": config.registry_program.to_string(),
                    "signals_program": config.signals_program.to_string(),
                    "keypair": config.default_keypair.display().to_string(),
                }),
                &[
                    format!("config_path: {}", config.config_path.display()),
                    format!("rpc_url: {}", config.rpc_url),
                    format!("token_program: {}", config.token_program),
                    format!("registry_program: {}", config.registry_program),
                    format!("signals_program: {}", config.signals_program),
                    format!("keypair: {}", config.default_keypair.display()),
                ],
            );
        }
    }

    Ok(())
}

async fn run_query(config: &AppConfig, command: QueryCommand) -> Result<()> {
    let query = QueryClient::new(
        HttpBackend::new(&config.rpc_url),
        config.token_program,
        config.registry_program,
    )
    .with_signals_program(config.signals_program);

    match command.command {
        QuerySubcommand::TokenConfig => {
            let token_config = query.token_config().await?;
            print_value(
                config.json,
                json!({
                    "authority": token_config.authority.to_string(),
                    "total_supply": token_config.total_supply,
                    "decimals": token_config.decimals,
                    "bump": token_config.bump,
                }),
                &[
                    format!("authority: {}", token_config.authority),
                    format!("total_supply: {}", token_config.total_supply),
                    format!("decimals: {}", token_config.decimals),
                    format!("bump: {}", token_config.bump),
                ],
            );
        }
        QuerySubcommand::Balance { owner } => {
            let owner = parse_pubkey(&owner, "owner pubkey")?;
            let balance = query.balance(&owner).await?;
            print_value(
                config.json,
                json!({
                    "owner": owner.to_string(),
                    "balance": balance,
                }),
                &[format!("owner: {owner}"), format!("balance: {balance}")],
            );
        }
        QuerySubcommand::RegistryConfig => {
            let registry_config = query.registry_config().await?;
            print_value(
                config.json,
                json!({
                    "price": registry_config.price,
                    "token_program": registry_config.token_program.to_string(),
                    "bump": registry_config.bump,
                }),
                &[
                    format!("price: {}", registry_config.price),
                    format!("token_program: {}", registry_config.token_program),
                    format!("bump: {}", registry_config.bump),
                ],
            );
        }
        QuerySubcommand::SignalsConfig => {
            let signals_config = query.signals_config().await?;
            print_value(
                config.json,
                json!({
                    "authority": signals_config.authority.to_string(),
                    "token_program": signals_config.token_program.to_string(),
                    "min_balance": signals_config.min_balance,
                    "bump": signals_config.bump,
                }),
                &[
                    format!("authority: {}", signals_config.authority),
                    format!("token_program: {}", signals_config.token_program),
                    format!("min_balance: {}", signals_config.min_balance),
                    format!("bump: {}", signals_config.bump),
                ],
            );
        }
        QuerySubcommand::NameRecord { name } => {
            let record = query.name_record(&name).await?;
            print_name_record(config.json, &name, record.as_ref());
        }
    }

    Ok(())
}

async fn run_token(config: &AppConfig, command: TokenCommand) -> Result<()> {
    match command.command {
        TokenSubcommand::Initialize {
            keypair,
            total_supply,
            decimals,
        } => {
            let signer = load_local_signer(config, &keypair)?;
            let authority = signer.pubkey();
            ensure_signer_account_exists(config, &authority).await?;
            let surf = Surf::new(
                HttpBackend::new(&config.rpc_url),
                config.token_program,
                config.registry_program,
            )
            .with_signals_program(config.signals_program);
            surf.authority(signer)
                .token()
                .initialize(total_supply, decimals)
                .await?;

            print_value(
                config.json,
                json!({
                    "status": "ok",
                    "authority": authority.to_string(),
                    "total_supply": total_supply,
                    "decimals": decimals,
                }),
                &[
                    "status: ok".to_string(),
                    format!("authority: {authority}"),
                    format!("total_supply: {total_supply}"),
                    format!("decimals: {decimals}"),
                ],
            );
        }
        TokenSubcommand::Mint {
            keypair,
            recipient,
            amount,
        } => {
            let signer = load_local_signer(config, &keypair)?;
            let authority = signer.pubkey();
            ensure_signer_account_exists(config, &authority).await?;
            let recipient = parse_pubkey(&recipient, "recipient pubkey")?;
            let surf = Surf::new(
                HttpBackend::new(&config.rpc_url),
                config.token_program,
                config.registry_program,
            )
            .with_signals_program(config.signals_program);
            surf.authority(signer)
                .token()
                .mint(&recipient, amount)
                .await?;

            print_value(
                config.json,
                json!({
                    "status": "ok",
                    "authority": authority.to_string(),
                    "recipient": recipient.to_string(),
                    "amount": amount,
                }),
                &[
                    "status: ok".to_string(),
                    format!("authority: {authority}"),
                    format!("recipient: {recipient}"),
                    format!("amount: {amount}"),
                ],
            );
        }
        TokenSubcommand::Transfer {
            keypair,
            recipient,
            amount,
        } => {
            let signer = load_local_signer(config, &keypair)?;
            let sender = signer.pubkey();
            ensure_signer_account_exists(config, &sender).await?;
            let recipient = parse_pubkey(&recipient, "recipient pubkey")?;
            let surf = Surf::new(
                HttpBackend::new(&config.rpc_url),
                config.token_program,
                config.registry_program,
            )
            .with_signals_program(config.signals_program);
            surf.user(signer)
                .token()
                .transfer(&recipient, amount)
                .await?;

            print_value(
                config.json,
                json!({
                    "status": "ok",
                    "sender": sender.to_string(),
                    "recipient": recipient.to_string(),
                    "amount": amount,
                }),
                &[
                    "status: ok".to_string(),
                    format!("sender: {sender}"),
                    format!("recipient: {recipient}"),
                    format!("amount: {amount}"),
                ],
            );
        }
        TokenSubcommand::Burn { keypair, amount } => {
            let signer = load_local_signer(config, &keypair)?;
            let holder = signer.pubkey();
            ensure_signer_account_exists(config, &holder).await?;
            let surf = Surf::new(
                HttpBackend::new(&config.rpc_url),
                config.token_program,
                config.registry_program,
            )
            .with_signals_program(config.signals_program);
            surf.user(signer).token().burn(amount).await?;

            print_value(
                config.json,
                json!({
                    "status": "ok",
                    "holder": holder.to_string(),
                    "amount": amount,
                }),
                &[
                    "status: ok".to_string(),
                    format!("holder: {holder}"),
                    format!("amount: {amount}"),
                ],
            );
        }
    }

    Ok(())
}

async fn run_names(config: &AppConfig, command: NamesCommand) -> Result<()> {
    match command.command {
        NamesSubcommand::Initialize { keypair, price } => {
            let signer = load_local_signer(config, &keypair)?;
            let authority = signer.pubkey();
            ensure_signer_account_exists(config, &authority).await?;
            let surf = Surf::new(
                HttpBackend::new(&config.rpc_url),
                config.token_program,
                config.registry_program,
            )
            .with_signals_program(config.signals_program);
            surf.authority(signer)
                .registry()
                .initialize(price, &config.token_program)
                .await?;

            print_value(
                config.json,
                json!({
                    "status": "ok",
                    "authority": authority.to_string(),
                    "price": price,
                    "token_program": config.token_program.to_string(),
                }),
                &[
                    "status: ok".to_string(),
                    format!("authority: {authority}"),
                    format!("price: {price}"),
                    format!("token_program: {}", config.token_program),
                ],
            );
        }
        NamesSubcommand::Register { keypair, name } => {
            let signer = load_local_signer(config, &keypair)?;
            let owner = signer.pubkey();
            ensure_signer_account_exists(config, &owner).await?;
            let surf = Surf::new(
                HttpBackend::new(&config.rpc_url),
                config.token_program,
                config.registry_program,
            )
            .with_signals_program(config.signals_program);
            surf.user(signer).names().register(&name).await?;

            print_value(
                config.json,
                json!({
                    "status": "ok",
                    "owner": owner.to_string(),
                    "name": normalize_name(&name)?,
                }),
                &[
                    "status: ok".to_string(),
                    format!("owner: {owner}"),
                    format!("name: {}", normalize_name(&name)?),
                ],
            );
        }
        NamesSubcommand::List => {
            let backend = HttpBackend::new(&config.rpc_url);
            let accounts = backend
                .get_program_accounts(
                    &config.registry_program,
                    Some(ProgramAccountsFilter {
                        data_size: Some(surf_protocol::NameRecord::LEN),
                    }),
                )
                .await?;

            let mut records = accounts
                .into_iter()
                .filter_map(|account| {
                    surf_protocol::decode_name_record(&account.account.data).map(|record| {
                        json!({
                            "account": account.pubkey.to_string(),
                            "owner": record.owner.to_string(),
                            "name": decode_name(&record),
                            "len": record.len,
                        })
                    })
                })
                .collect::<Vec<_>>();

            records.sort_by(|left, right| left["name"].as_str().cmp(&right["name"].as_str()));

            if config.json {
                print_value(true, Value::Array(records), &[]);
            } else if records.is_empty() {
                println!("no names found");
            } else {
                for record in &records {
                    println!(
                        "{} -> {}",
                        record["name"].as_str().unwrap_or(""),
                        record["owner"].as_str().unwrap_or("")
                    );
                }
            }
        }
        NamesSubcommand::Lookup { name } => {
            let query = QueryClient::new(
                HttpBackend::new(&config.rpc_url),
                config.token_program,
                config.registry_program,
            )
            .with_signals_program(config.signals_program);
            let record = query.name_record(&name).await?;
            print_name_record(config.json, &name, record.as_ref());
        }
    }

    Ok(())
}

async fn run_sol(config: &AppConfig, command: SolCommand) -> Result<()> {
    match command.command {
        SolSubcommand::Transfer {
            keypair,
            recipient,
            lamports,
        } => {
            let signer = load_local_signer(config, &keypair)?;
            let sender = signer.pubkey();
            ensure_signer_account_exists(config, &sender).await?;
            let recipient = parse_pubkey(&recipient, "recipient pubkey")?;

            let backend = HttpBackend::new(&config.rpc_url);
            let blockhash = backend.get_latest_blockhash().await?;
            let instruction = sol_transfer_instruction(&sender, &recipient, lamports);
            let message = Message::new_with_blockhash(&[instruction], Some(&sender), &blockhash);
            let mut tx = Transaction::new_unsigned(message);
            tx.sign(&[&signer], blockhash);
            backend.send_and_confirm(&tx).await?;

            print_value(
                config.json,
                json!({
                    "status": "ok",
                    "sender": sender.to_string(),
                    "recipient": recipient.to_string(),
                    "lamports": lamports,
                }),
                &[
                    "status: ok".to_string(),
                    format!("sender: {sender}"),
                    format!("recipient: {recipient}"),
                    format!("lamports: {lamports}"),
                ],
            );
        }
    }

    Ok(())
}

async fn run_signals(config: &AppConfig, command: SignalsCommand) -> Result<()> {
    match command.command {
        SignalsSubcommand::Initialize { keypair } => {
            let signer = load_local_signer(config, &keypair)?;
            let authority = signer.pubkey();
            ensure_signer_account_exists(config, &authority).await?;
            let surf = Surf::new(
                HttpBackend::new(&config.rpc_url),
                config.token_program,
                config.registry_program,
            )
            .with_signals_program(config.signals_program);
            surf.authority(signer)
                .signals()
                .initialize(1, &config.token_program)
                .await?;

            print_value(
                config.json,
                json!({
                    "status": "ok",
                    "authority": authority.to_string(),
                    "signals_program": config.signals_program.to_string(),
                    "token_program": config.token_program.to_string(),
                    "min_balance": 1,
                }),
                &[
                    "status: ok".to_string(),
                    format!("authority: {authority}"),
                    format!("signals_program: {}", config.signals_program),
                    format!("token_program: {}", config.token_program),
                    "min_balance: 1".to_string(),
                ],
            );
        }
        SignalsSubcommand::Follow { keypair, target } => {
            let signer = load_local_signer(config, &keypair)?;
            let sender = signer.pubkey();
            ensure_signer_account_exists(config, &sender).await?;
            let target = parse_pubkey(&target, "target pubkey")?;
            send_signal(config, signer, sender, target, None, true).await?;
        }
        SignalsSubcommand::FollowName { keypair, name } => {
            let signer = load_local_signer(config, &keypair)?;
            let sender = signer.pubkey();
            ensure_signer_account_exists(config, &sender).await?;
            let normalized_name = normalize_name(&name)?;
            let target = lookup_name_owner(config, &normalized_name, sender).await?;
            send_signal(config, signer, sender, target, Some(normalized_name), true).await?;
        }
        SignalsSubcommand::Unfollow { keypair, target } => {
            let signer = load_local_signer(config, &keypair)?;
            let sender = signer.pubkey();
            ensure_signer_account_exists(config, &sender).await?;
            let target = parse_pubkey(&target, "target pubkey")?;
            send_signal(config, signer, sender, target, None, false).await?;
        }
        SignalsSubcommand::UnfollowName { keypair, name } => {
            let signer = load_local_signer(config, &keypair)?;
            let sender = signer.pubkey();
            ensure_signer_account_exists(config, &sender).await?;
            let normalized_name = normalize_name(&name)?;
            let target = lookup_name_owner(config, &normalized_name, sender).await?;
            send_signal(config, signer, sender, target, Some(normalized_name), false).await?;
        }
    }

    Ok(())
}

async fn send_signal(
    config: &AppConfig,
    signer: LocalKeypairSigner,
    sender: Pubkey,
    target: Pubkey,
    name: Option<String>,
    follow: bool,
) -> Result<()> {
    if sender == target {
        let subject = name.as_deref().unwrap_or("that account");
        let action = if follow { "follow" } else { "unfollow" };
        return Err(Box::new(CliError(format!(
            "cannot {action} yourself via {subject}"
        ))));
    }

    let surf = Surf::new(
        HttpBackend::new(&config.rpc_url),
        config.token_program,
        config.registry_program,
    )
    .with_signals_program(config.signals_program);

    if follow {
        surf.user(signer).signals().follow(&target).await?;
    } else {
        surf.user(signer).signals().unfollow(&target).await?;
    }

    let action = if follow { "follow" } else { "unfollow" };
    let mut value = json!({
        "status": "ok",
        "action": action,
        "sender": sender.to_string(),
        "target": target.to_string(),
    });
    let mut lines = vec![
        "status: ok".to_string(),
        format!("action: {action}"),
        format!("sender: {sender}"),
        format!("target: {target}"),
    ];

    if let Some(name) = name {
        value["name"] = Value::String(name.clone());
        lines.push(format!("name: {name}"));
    }

    print_value(config.json, value, &lines);
    Ok(())
}

async fn lookup_name_owner(config: &AppConfig, name: &str, sender: Pubkey) -> Result<Pubkey> {
    let query = QueryClient::new(
        HttpBackend::new(&config.rpc_url),
        config.token_program,
        config.registry_program,
    )
    .with_signals_program(config.signals_program);
    let record = query.name_record(name).await?;

    match record {
        Some(record) if record.owner == sender => Err(Box::new(CliError(format!(
            "name '{name}' resolves to your own account; use another user name"
        )))),
        Some(record) => Ok(record.owner),
        None => Err(Box::new(CliError(format!(
            "name '{name}' was not found in the SURF registry"
        )))),
    }
}

fn parse_pubkey(value: &str, label: &str) -> Result<Pubkey> {
    value
        .parse()
        .map_err(|_| Box::new(CliError(format!("invalid {label}: {value}"))) as Box<dyn StdError>)
}

fn resolve_program_id(
    flag_value: Option<&str>,
    file_value: Option<&str>,
    env_key: Option<&str>,
    label: &str,
    flag_name: &str,
) -> Result<Pubkey> {
    let value = resolve_string_option(
        flag_value,
        file_value,
        env_key,
        None,
        label,
        Some(flag_name),
    )?;

    parse_pubkey(&value, label)
}

fn resolve_string_option(
    flag_value: Option<&str>,
    file_value: Option<&str>,
    env_key: Option<&str>,
    default_value: Option<String>,
    label: &str,
    flag_name: Option<&str>,
) -> Result<String> {
    if let Some(value) = flag_value {
        return Ok(value.to_string());
    }

    if let Some(value) = file_value {
        return Ok(value.to_string());
    }

    if let Some(env_key) = env_key {
        if let Ok(value) = env::var(env_key) {
            return Ok(value);
        }
    }

    if let Some(value) = default_value {
        return Ok(value);
    }

    let mut sources = Vec::new();
    if let Some(flag_name) = flag_name {
        sources.push(format!("--{flag_name}"));
    }
    if let Some(env_key) = env_key {
        sources.push(env_key.to_string());
    }
    sources.push("the config file".to_string());

    Err(Box::new(CliError(format!(
        "missing {label}; provide {}",
        sources.join(", ")
    ))))
}

fn load_local_signer(config: &AppConfig, args: &KeypairArgs) -> Result<LocalKeypairSigner> {
    let path = match args.keypair.as_deref() {
        Some(path) => expand_tilde(path)?,
        None => config.default_keypair.clone(),
    };
    let keypair = Keypair::read_from_file(&path).map_err(|err| {
        Box::new(CliError(format!(
            "failed to read keypair {}: {err}",
            path.display()
        ))) as Box<dyn StdError>
    })?;
    Ok(LocalKeypairSigner::new(keypair))
}

async fn ensure_signer_account_exists(config: &AppConfig, pubkey: &Pubkey) -> Result<()> {
    let backend = HttpBackend::new(&config.rpc_url);
    let balance = backend.get_balance(pubkey).await?;

    if balance.is_none() {
        return Err(Box::new(CliError(format!(
            "signer account {pubkey} is missing on {}; fund it before sending transactions",
            config.rpc_url
        ))));
    }

    Ok(())
}

fn resolve_keypair_path(
    flag_value: Option<&str>,
    file_value: Option<&str>,
    env_key: Option<&str>,
    default_value: Option<String>,
) -> Result<PathBuf> {
    let value = resolve_string_option(
        flag_value,
        file_value,
        env_key,
        default_value,
        "keypair path",
        Some("keypair"),
    )?;
    expand_tilde(&value)
}

fn resolve_config_path(path: Option<&str>) -> Result<PathBuf> {
    let value = path.unwrap_or(DEFAULT_CONFIG_PATH);
    expand_tilde(value)
}

fn expand_tilde(path: &str) -> Result<PathBuf> {
    if path == "~" {
        return home_dir();
    }

    if let Some(rest) = path.strip_prefix("~/") {
        return Ok(home_dir()?.join(rest));
    }

    Ok(Path::new(path).to_path_buf())
}

fn home_dir() -> Result<PathBuf> {
    env::var("HOME")
        .map(PathBuf::from)
        .map_err(|_| Box::new(CliError("HOME is not set".to_string())) as Box<dyn StdError>)
}

fn normalize_name(name: &str) -> Result<String> {
    let normalized = surf_protocol::validate_name(name)
        .map_err(|err| Box::new(CliError(err.to_string())) as Box<dyn StdError>)?;
    let len = name.len();
    Ok(String::from_utf8_lossy(&normalized[..len]).into_owned())
}

fn decode_name(record: &surf_protocol::NameRecord) -> String {
    String::from_utf8_lossy(&record.name[..usize::from(record.len)]).into_owned()
}

fn print_name_record(
    json_output: bool,
    requested_name: &str,
    record: Option<&surf_protocol::NameRecord>,
) {
    match record {
        Some(record) => {
            print_value(
                json_output,
                json!({
                    "requested_name": requested_name,
                    "found": true,
                    "owner": record.owner.to_string(),
                    "name": decode_name(record),
                    "len": record.len,
                }),
                &[
                    format!("requested_name: {requested_name}"),
                    "found: true".to_string(),
                    format!("owner: {}", record.owner),
                    format!("name: {}", decode_name(record)),
                    format!("len: {}", record.len),
                ],
            );
        }
        None => {
            print_value(
                json_output,
                json!({
                    "requested_name": requested_name,
                    "found": false,
                }),
                &[
                    format!("requested_name: {requested_name}"),
                    "found: false".to_string(),
                ],
            );
        }
    }
}

fn print_value(json_output: bool, value: Value, lines: &[String]) {
    if json_output {
        println!(
            "{}",
            serde_json::to_string_pretty(&value).expect("json serialization should succeed")
        );
        return;
    }

    for line in lines {
        println!("{line}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn expands_tilde_path() {
        let home = env::var("HOME").expect("HOME should be set for tests");
        let path = expand_tilde("~/.config/solana/id.json").expect("tilde expansion should work");
        assert_eq!(path, PathBuf::from(home).join(".config/solana/id.json"));
    }

    #[test]
    fn normalizes_name_to_lowercase() {
        let normalized = normalize_name("AlIcE").expect("name should normalize");
        assert_eq!(normalized, "alice");
    }

    #[test]
    fn decodes_name_record_text() {
        let mut name = [0u8; 32];
        name[..5].copy_from_slice(b"alice");
        let record = surf_protocol::NameRecord {
            owner: Pubkey::new_unique(),
            name,
            len: 5,
        };

        assert_eq!(decode_name(&record), "alice");
    }

    #[test]
    fn loads_program_ids_from_config_file() {
        let temp_path = temp_config_path("config-load");
        let token_program = Pubkey::new_unique();
        let registry_program = Pubkey::new_unique();
        let signals_program = Pubkey::new_unique();
        let token_program_string = token_program.to_string();
        let registry_program_string = registry_program.to_string();
        let signals_program_string = signals_program.to_string();
        let rpc_url = "http://127.0.0.1:8899";
        let keypair = "~/.config/solana/id.json";

        fs::write(
            &temp_path,
            format!(
                "{{\n  \"rpc_url\": \"{}\",\n  \"token_program\": \"{}\",\n  \"registry_program\": \"{}\",\n  \"signals_program\": \"{}\",\n  \"keypair\": \"{}\"\n}}",
                rpc_url, token_program, registry_program, signals_program, keypair
            ),
        )
        .expect("config file should be written");

        let config = FileConfig::load(Some(temp_path.to_str().expect("path should be utf-8")))
            .expect("config should load");

        assert_eq!(config.rpc_url.as_deref(), Some(rpc_url));
        assert_eq!(
            config.token_program.as_deref(),
            Some(token_program_string.as_str())
        );
        assert_eq!(
            config.registry_program.as_deref(),
            Some(registry_program_string.as_str())
        );
        assert_eq!(
            config.signals_program.as_deref(),
            Some(signals_program_string.as_str())
        );
        assert_eq!(config.keypair.as_deref(), Some(keypair));

        let _ = fs::remove_file(temp_path);
    }

    #[test]
    fn resolves_program_id_from_file_when_flag_and_env_missing() {
        env::remove_var(ENV_TOKEN_PROGRAM_ID);
        let token_program = Pubkey::new_unique();
        let token_program_string = token_program.to_string();

        let resolved = resolve_program_id(
            None,
            Some(token_program_string.as_str()),
            Some(ENV_TOKEN_PROGRAM_ID),
            "token program id",
            "token-program",
        )
        .expect("program id should resolve from file");

        assert_eq!(resolved, token_program);
    }

    #[test]
    fn resolves_keypair_path_from_file_when_flag_missing() {
        let path = resolve_keypair_path(
            None,
            Some("~/.config/solana/id.json"),
            Some(ENV_KEYPAIR),
            Some(DEFAULT_KEYPAIR.to_string()),
        )
        .expect("keypair path should resolve");

        let home = env::var("HOME").expect("HOME should be set for tests");
        assert_eq!(path, PathBuf::from(home).join(".config/solana/id.json"));
    }

    fn temp_config_path(prefix: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after unix epoch")
            .as_nanos();
        env::temp_dir().join(format!("surf-cli-{prefix}-{unique}.json"))
    }
}
