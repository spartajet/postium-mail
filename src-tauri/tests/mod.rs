mod config_tests;
mod diagnose_folder_sync;
mod smtp_integrate_test;
mod test_163_folder_names;
mod test_163_full_emails;
mod test_decode_subject;
mod test_folder_attrs;
mod test_garbled_subject;
mod test_raw_email;
mod test_rfc6154;
mod test_sync_since;
mod test_uid_2408;

// 集成测试模块（需要 Docker GreenMail）
pub mod integration;
