use clap::Parser;
use shared_proto::{Greeting, User};

#[derive(Parser, Debug)]
#[command(name = "surf")]
#[command(about = "Surf CLI tool", long_about = None)]
struct Args {
    #[arg(short, long, help = "Path to output file")]
    output: Option<String>,

    #[arg(short, long, help = "Print greeting message")]
    greet: bool,
}

fn main() {
    let args = Args::parse();

    let user = User {
        id: "1".to_string(),
        name: "Test User".to_string(),
        email: "test@example.com".to_string(),
    };

    if args.greet {
        let greeting = Greeting {
            message: "Hello from Surf CLI!".to_string(),
            user: Some(user.clone()),
        };
        println!("Message: {}", greeting.message);
        if let Some(u) = greeting.user {
            println!("User: {} <{}>", u.name, u.email);
        }
    } else {
        println!("User: {} <{}>", user.name, user.email);
    }

    if let Some(path) = args.output {
        let encoded = prost::Message::encode_to_vec(&user);
        let _ = std::fs::write(&path, encoded);
        println!("User written to {}", path);
    }
}
