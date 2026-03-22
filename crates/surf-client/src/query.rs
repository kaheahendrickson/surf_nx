use solana_pubkey::Pubkey;

#[cfg(not(target_arch = "wasm32"))]
use crate::backend::Backend;
use crate::error::Error;

pub struct QueryClient<B> {
    backend: B,
    token_program: Pubkey,
    registry_program: Pubkey,
    signals_program: Pubkey,
}

#[cfg(not(target_arch = "wasm32"))]
impl<B> QueryClient<B>
where
    B: Backend,
{
    pub fn new(backend: B, token_program: Pubkey, registry_program: Pubkey) -> Self {
        Self {
            backend,
            token_program,
            registry_program,
            signals_program: Pubkey::default(),
        }
    }

    pub fn with_signals_program(mut self, signals_program: Pubkey) -> Self {
        self.signals_program = signals_program;
        self
    }

    pub async fn token_config(&self) -> Result<surf_protocol::TokenConfig, Error> {
        let (config_pda, _) = surf_protocol::derive_token_config_pda(&self.token_program);
        let account = self
            .backend
            .get_account(&config_pda)
            .await?
            .ok_or(Error::AccountNotFound(config_pda))?;
        surf_protocol::decode_token_config(&account.data).ok_or(Error::InvalidAccountData)
    }

    pub async fn balance(&self, owner: &Pubkey) -> Result<u64, Error> {
        let (balance_pda, _) = surf_protocol::derive_token_balance_pda(owner, &self.token_program);
        let account = self
            .backend
            .get_account(&balance_pda)
            .await?
            .ok_or(Error::AccountNotFound(balance_pda))?;
        let balance =
            surf_protocol::decode_token_balance(&account.data).ok_or(Error::InvalidAccountData)?;
        Ok(balance.amount)
    }

    pub async fn name_record(
        &self,
        name: &str,
    ) -> Result<Option<surf_protocol::NameRecord>, Error> {
        let normalized =
            surf_protocol::validate_name(name).map_err(|e| Error::Validation(e.to_string()))?;
        let name_len = name.len();
        let (name_pda, _) =
            surf_protocol::derive_name_record_pda(&normalized[..name_len], &self.registry_program);

        match self.backend.get_account(&name_pda).await? {
            Some(account) => {
                let record = surf_protocol::decode_name_record(&account.data)
                    .ok_or(Error::InvalidAccountData)?;
                Ok(Some(record))
            }
            None => Ok(None),
        }
    }

    pub async fn registry_config(&self) -> Result<surf_protocol::RegistryConfig, Error> {
        let (config_pda, _) = surf_protocol::derive_registry_config_pda(&self.registry_program);
        let account = self
            .backend
            .get_account(&config_pda)
            .await?
            .ok_or(Error::AccountNotFound(config_pda))?;
        surf_protocol::decode_registry_config(&account.data).ok_or(Error::InvalidAccountData)
    }

    pub async fn signals_config(&self) -> Result<surf_protocol::SignalsConfig, Error> {
        if self.signals_program == Pubkey::default() {
            return Err(Error::Validation(
                "signals program is not configured on this client".to_string(),
            ));
        }

        let (config_pda, _) = surf_protocol::derive_signals_config_pda(&self.signals_program);
        let account = self
            .backend
            .get_account(&config_pda)
            .await?
            .ok_or(Error::AccountNotFound(config_pda))?;
        surf_protocol::decode_signals_config(&account.data).ok_or(Error::InvalidAccountData)
    }
}

#[cfg(target_arch = "wasm32")]
impl<B> QueryClient<B>
where
    B: crate::backend::WasmBackend,
{
    pub fn new(backend: B, token_program: Pubkey, registry_program: Pubkey) -> Self {
        Self {
            backend,
            token_program,
            registry_program,
            signals_program: Pubkey::default(),
        }
    }

    pub fn with_signals_program(mut self, signals_program: Pubkey) -> Self {
        self.signals_program = signals_program;
        self
    }

    pub async fn token_config(&self) -> Result<surf_protocol::TokenConfig, Error> {
        let (config_pda, _) = surf_protocol::derive_token_config_pda(&self.token_program);
        let account = self
            .backend
            .get_account(&config_pda)
            .await?
            .ok_or(Error::AccountNotFound(config_pda))?;
        surf_protocol::decode_token_config(&account.data).ok_or(Error::InvalidAccountData)
    }

    pub async fn balance(&self, owner: &Pubkey) -> Result<u64, Error> {
        let (balance_pda, _) = surf_protocol::derive_token_balance_pda(owner, &self.token_program);
        let account = self
            .backend
            .get_account(&balance_pda)
            .await?
            .ok_or(Error::AccountNotFound(balance_pda))?;
        let balance =
            surf_protocol::decode_token_balance(&account.data).ok_or(Error::InvalidAccountData)?;
        Ok(balance.amount)
    }

    pub async fn name_record(
        &self,
        name: &str,
    ) -> Result<Option<surf_protocol::NameRecord>, Error> {
        let normalized =
            surf_protocol::validate_name(name).map_err(|e| Error::Validation(e.to_string()))?;
        let name_len = name.len();
        let (name_pda, _) =
            surf_protocol::derive_name_record_pda(&normalized[..name_len], &self.registry_program);

        match self.backend.get_account(&name_pda).await? {
            Some(account) => {
                let record = surf_protocol::decode_name_record(&account.data)
                    .ok_or(Error::InvalidAccountData)?;
                Ok(Some(record))
            }
            None => Ok(None),
        }
    }

    pub async fn registry_config(&self) -> Result<surf_protocol::RegistryConfig, Error> {
        let (config_pda, _) = surf_protocol::derive_registry_config_pda(&self.registry_program);
        let account = self
            .backend
            .get_account(&config_pda)
            .await?
            .ok_or(Error::AccountNotFound(config_pda))?;
        surf_protocol::decode_registry_config(&account.data).ok_or(Error::InvalidAccountData)
    }

    pub async fn signals_config(&self) -> Result<surf_protocol::SignalsConfig, Error> {
        if self.signals_program == Pubkey::default() {
            return Err(Error::Validation(
                "signals program is not configured on this client".to_string(),
            ));
        }

        let (config_pda, _) = surf_protocol::derive_signals_config_pda(&self.signals_program);
        let account = self
            .backend
            .get_account(&config_pda)
            .await?
            .ok_or(Error::AccountNotFound(config_pda))?;
        surf_protocol::decode_signals_config(&account.data).ok_or(Error::InvalidAccountData)
    }
}
