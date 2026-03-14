pub mod account;
pub mod email;
pub mod attachment;

pub use account::{Model as Account, Entity as AccountEntity};
pub use email::{Model as Email, Entity as EmailEntity};
pub use attachment::{Model as Attachment, Entity as AttachmentEntity};
