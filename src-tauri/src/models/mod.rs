pub mod account;
pub mod email;
pub mod attachment;
pub mod folder;
pub mod sync_state;
pub mod sync_error;

pub use account::{Model as Account, Entity as AccountEntity};
pub use email::{Model as Email, Entity as EmailEntity};
pub use attachment::{Model as Attachment, Entity as AttachmentEntity};
pub use folder::{Model as Folder, Entity as FolderEntity};
pub use sync_state::{Model as SyncState, Entity as SyncStateEntity};
pub use sync_error::{Model as SyncError, Entity as SyncErrorEntity};
