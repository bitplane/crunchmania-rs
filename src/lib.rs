pub mod bitreader;
pub mod constants;
pub mod error;
pub mod header;
pub mod pack;
pub mod unpack;

pub use error::CrmError;
pub use header::{parse_header, CrmHeader};
pub use pack::pack;
pub use unpack::unpack;
