use std::marker::PhantomData;

use solana_pubkey::Pubkey;
use solana_signer::Signer;

#[cfg(not(target_arch = "wasm32"))]
use crate::backend::Backend;
use crate::names::NamesClient;
use crate::role::{AuthorityRole, HarnessRole, NoSigner, Sealed, UserRole};
use crate::signals::SignalsClient;
use crate::token::TokenClient;

pub struct Surf<B> {
    backend: B,
    token_program: Pubkey,
    registry_program: Pubkey,
    signals_program: Pubkey,
}

#[cfg(not(target_arch = "wasm32"))]
impl<B> Surf<B>
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

    pub fn authority<S: Signer>(self, signer: S) -> SurfClient<B, AuthorityRole, S> {
        SurfClient {
            backend: self.backend,
            token_program: self.token_program,
            registry_program: self.registry_program,
            signals_program: self.signals_program,
            signer,
            _role: PhantomData,
        }
    }

    pub fn user<S: Signer>(self, signer: S) -> SurfClient<B, UserRole, S> {
        SurfClient {
            backend: self.backend,
            token_program: self.token_program,
            registry_program: self.registry_program,
            signals_program: self.signals_program,
            signer,
            _role: PhantomData,
        }
    }

    pub fn harness(self) -> SurfClient<B, HarnessRole, NoSigner> {
        SurfClient {
            backend: self.backend,
            token_program: self.token_program,
            registry_program: self.registry_program,
            signals_program: self.signals_program,
            signer: NoSigner,
            _role: PhantomData,
        }
    }
}

#[cfg(target_arch = "wasm32")]
impl<B> Surf<B>
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

    pub fn authority<S: Signer>(self, signer: S) -> SurfClient<B, AuthorityRole, S> {
        SurfClient {
            backend: self.backend,
            token_program: self.token_program,
            registry_program: self.registry_program,
            signals_program: self.signals_program,
            signer,
            _role: PhantomData,
        }
    }

    pub fn user<S: Signer>(self, signer: S) -> SurfClient<B, UserRole, S> {
        SurfClient {
            backend: self.backend,
            token_program: self.token_program,
            registry_program: self.registry_program,
            signals_program: self.signals_program,
            signer,
            _role: PhantomData,
        }
    }

    pub fn harness(self) -> SurfClient<B, HarnessRole, NoSigner> {
        SurfClient {
            backend: self.backend,
            token_program: self.token_program,
            registry_program: self.registry_program,
            signals_program: self.signals_program,
            signer: NoSigner,
            _role: PhantomData,
        }
    }
}

pub struct SurfClient<B, R, S> {
    backend: B,
    token_program: Pubkey,
    registry_program: Pubkey,
    signals_program: Pubkey,
    signer: S,
    _role: PhantomData<R>,
}

impl<B, R, S> SurfClient<B, R, S>
where
    R: Sealed,
{
    pub fn token_program(&self) -> &Pubkey {
        &self.token_program
    }

    pub fn registry_program(&self) -> &Pubkey {
        &self.registry_program
    }

    pub fn signals_program(&self) -> Option<&Pubkey> {
        if self.signals_program == Pubkey::default() {
            None
        } else {
            Some(&self.signals_program)
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl<B, R, S> SurfClient<B, R, S>
where
    B: Backend,
    R: Sealed,
    S: Signer,
{
    pub fn signer_pubkey(&self) -> Pubkey {
        self.signer.pubkey()
    }

    pub fn token(&self) -> TokenClient<'_, B, R, S> {
        TokenClient::new(
            &self.backend,
            &self.token_program,
            &self.registry_program,
            &self.signer,
        )
    }

    pub fn names(&self) -> NamesClient<'_, B, R, S> {
        NamesClient::new(
            &self.backend,
            &self.token_program,
            &self.registry_program,
            &self.signer,
        )
    }

    pub fn signals(&self) -> SignalsClient<'_, B, R, S> {
        SignalsClient::new(
            &self.backend,
            &self.token_program,
            &self.signals_program,
            &self.signer,
        )
    }
}

#[cfg(target_arch = "wasm32")]
impl<B, R, S> SurfClient<B, R, S>
where
    B: crate::backend::WasmBackend,
    R: Sealed,
    S: Signer,
{
    pub fn signer_pubkey(&self) -> Pubkey {
        self.signer.pubkey()
    }

    pub fn token(&self) -> TokenClient<'_, B, R, S> {
        TokenClient::new(
            &self.backend,
            &self.token_program,
            &self.registry_program,
            &self.signer,
        )
    }

    pub fn names(&self) -> NamesClient<'_, B, R, S> {
        NamesClient::new(
            &self.backend,
            &self.token_program,
            &self.registry_program,
            &self.signer,
        )
    }

    pub fn signals(&self) -> SignalsClient<'_, B, R, S> {
        SignalsClient::new(
            &self.backend,
            &self.token_program,
            &self.signals_program,
            &self.signer,
        )
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl<B, S> SurfClient<B, AuthorityRole, S>
where
    B: Backend,
    S: Signer,
{
    pub fn registry(&self) -> NamesClient<'_, B, AuthorityRole, S> {
        self.names()
    }
}

#[cfg(target_arch = "wasm32")]
impl<B, S> SurfClient<B, AuthorityRole, S>
where
    B: crate::backend::WasmBackend,
    S: Signer,
{
    pub fn registry(&self) -> NamesClient<'_, B, AuthorityRole, S> {
        self.names()
    }
}

pub type AuthorityClient<B, S> = SurfClient<B, AuthorityRole, S>;
pub type UserClient<B, S> = SurfClient<B, UserRole, S>;
pub type HarnessClient<B> = SurfClient<B, HarnessRole, NoSigner>;
