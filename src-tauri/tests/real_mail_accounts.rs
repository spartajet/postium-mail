mod common;

use common::real_mail::{TruthAccount, imap_server_config, load_truth_accounts, mask_email};
use postium_mail_lib::error::MailError;
use postium_mail_lib::infrastructure::protocols::imap::ImapClient;
use tokio::time::{Duration, timeout};

const PROTOCOL_TIMEOUT: Duration = Duration::from_secs(30);

#[tokio::test]
#[ignore = "truth 测试会连接真实 IMAP 服务"]
async fn truth_real_mail_accounts_should_support_imap() {
    let accounts = load_truth_accounts();

    for account in &accounts {
        verify_imap(account).await.unwrap_or_else(|err| {
            panic!(
                "truth account {} ({}) IMAP 验证失败: {err}",
                account.key,
                mask_email(&account.email)
            )
        });
    }
}

async fn verify_imap(account: &TruthAccount) -> Result<(), MailError> {
    let config = imap_server_config(account);
    let mut client = timeout(
        PROTOCOL_TIMEOUT,
        ImapClient::connect(&config, &account.email, &account.password),
    )
    .await
    .map_err(|_| {
        MailError::ImapConnectionFailed(format!("IMAP 连接超时 {}:{}", config.host, config.port))
    })??;

    let folders = timeout(PROTOCOL_TIMEOUT, client.list_folders())
        .await
        .map_err(|_| MailError::ImapConnectionFailed("IMAP 列文件夹超时".to_string()))??;
    assert!(
        !folders.is_empty(),
        "truth account {} IMAP folder list 不能为空",
        account.key
    );

    let inbox = folders
        .iter()
        .find(|folder| folder.name.eq_ignore_ascii_case("INBOX"))
        .map(|folder| folder.name.clone())
        .unwrap_or_else(|| folders[0].name.clone());

    timeout(PROTOCOL_TIMEOUT, client.select_folder(&inbox))
        .await
        .map_err(|_| MailError::ImapConnectionFailed(format!("IMAP 选择文件夹 {inbox} 超时")))??;

    if let Err(err) = client.logout().await {
        tracing::debug!(error = %err, "truth IMAP logout 失败，忽略");
    }

    Ok(())
}
