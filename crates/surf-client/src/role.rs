mod sealed {
    pub trait Sealed {}
}

pub use sealed::Sealed;

pub struct AuthorityRole;
pub struct UserRole;
pub struct HarnessRole;

impl Sealed for AuthorityRole {}
impl Sealed for UserRole {}
impl Sealed for HarnessRole {}

pub trait CanInitToken: Sealed {}
pub trait CanMint: Sealed {}
pub trait CanInitRegistry: Sealed {}
pub trait CanTransfer: Sealed {}
pub trait CanBurn: Sealed {}
pub trait CanRegisterName: Sealed {}
pub trait CanInitSignals: Sealed {}
pub trait CanSendSignal: Sealed {}
pub trait CanAirdrop: Sealed {}
pub trait CanLoadPrograms: Sealed {}
pub trait CanQuery: Sealed {}

impl CanInitToken for AuthorityRole {}
impl CanMint for AuthorityRole {}
impl CanInitRegistry for AuthorityRole {}
impl CanInitSignals for AuthorityRole {}
impl CanQuery for AuthorityRole {}

impl CanTransfer for UserRole {}
impl CanBurn for UserRole {}
impl CanRegisterName for UserRole {}
impl CanSendSignal for UserRole {}
impl CanQuery for UserRole {}

impl CanAirdrop for HarnessRole {}
impl CanLoadPrograms for HarnessRole {}
impl CanQuery for HarnessRole {}

pub struct NoSigner;

impl NoSigner {
    pub fn pubkey(&self) -> ! {
        panic!("NoSigner has no pubkey")
    }
}
