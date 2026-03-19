pub mod account;
pub mod email;
pub mod attachment;
pub mod folder;
pub mod sync_state;
pub mod sync_error;

pub use account::Entity as AccountEntity;
pub use email::Entity as EmailEntity;
pub use attachment::Entity as AttachmentEntity;
pub use folder::Entity as FolderEntity;
pub use sync_state::Entity as SyncStateEntity;
pub use sync_error::Entity as SyncErrorEntity;
