//! Authentication adapters of kartenn (spec 005): the key ring of the instances, sealed
//! cookies, the secrets of service accounts, the OIDC flow of the BFF, access tokens and
//! break-glass accounts. They turn a proof into a principal; they decide no right.

pub mod breakglass;
pub mod http;
pub mod issuer;
pub mod keys;
pub mod oidc;
pub mod seal;
pub mod secrets;
pub mod tokens;
