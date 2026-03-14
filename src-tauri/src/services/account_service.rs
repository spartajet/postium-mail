use sea_orm::*;
use anyhow::{anyhow, Result};
use crate::models::{account, AccountEntity};
use crate::crypto::PasswordEncryptor;

/// 获取所有账号
pub async fn get_all(db: &DbConn) -> Result<Vec<account::AccountDto>> {
    let accounts = AccountEntity::find()
        .all(db)
        .await
        .map_err(|e| anyhow!("获取账号列表失败: {}", e))?;

    Ok(accounts.into_iter().map(|a| a.into()).collect())
}

/// 根据 ID 获取账号
pub async fn get_by_id(db: &DbConn, id: i32) -> Result<Option<account::Model>> {
    Ok(AccountEntity::find_by_id(id)
        .one(db)
        .await
        .map_err(|e| anyhow!("获取账号失败: {}", e))?)
}

/// 根据邮箱获取账号
pub async fn get_by_email(db: &DbConn, email: &str) -> Result<Option<account::Model>> {
    Ok(AccountEntity::find()
        .filter(account::Column::Email.eq(email))
        .one(db)
        .await
        .map_err(|e| anyhow!("获取账号失败: {}", e))?)
}

/// 创建新账号
pub async fn create(
    db: &DbConn,
    req: account::CreateAccountRequest,
) -> Result<account::Model> {
    // 检查邮箱是否已存在
    if let Some(_) = get_by_email(db, &req.email).await? {
        return Err(anyhow!("该邮箱地址已存在"));
    }

    // 加密密码
    let encryptor = PasswordEncryptor::new()?;
    let encrypted_password = encryptor.encrypt(&req.password)?;

    // 如果是预设服务商，填充默认配置
    let (imap_config, smtp_config) = account::get_provider_defaults(&req.provider)
        .unwrap_or_else(|| {
            (
                account::ImapConfig {
                    host: req.imap_host.clone().unwrap_or_default(),
                    port: req.imap_port.unwrap_or(993),
                    ssl: req.imap_ssl.unwrap_or(true),
                },
                account::SmtpConfig {
                    host: req.smtp_host.clone().unwrap_or_default(),
                    port: req.smtp_port.unwrap_or(587),
                    ssl: req.smtp_ssl.unwrap_or(true),
                },
            )
        });

    let now = chrono::Utc::now().timestamp();

    let new_account = account::ActiveModel {
        name: Set(req.name.clone()),
        email: Set(req.email.clone()),
        provider: Set(req.provider.clone()),
        imap_host: Set(if req.imap_host.is_some() {
            req.imap_host
        } else {
            Some(imap_config.host)
        }),
        imap_port: Set(if req.imap_port.is_some() {
            req.imap_port
        } else {
            Some(imap_config.port)
        }),
        imap_ssl: Set(Some(imap_config.ssl)),
        smtp_host: Set(if req.smtp_host.is_some() {
            req.smtp_host
        } else {
            Some(smtp_config.host)
        }),
        smtp_port: Set(if req.smtp_port.is_some() {
            req.smtp_port
        } else {
            Some(smtp_config.port)
        }),
        smtp_ssl: Set(Some(smtp_config.ssl)),
        password: Set(encrypted_password),
        color: Set(req.color),
        created_at: Set(now),
        updated_at: Set(now),
        ..Default::default()
    };

    let result = AccountEntity::insert(new_account)
        .exec(db)
        .await
        .map_err(|e| anyhow!("创建账号失败: {}", e))?;

    get_by_id(db, result.last_insert_id)
        .await?
        .ok_or_else(|| anyhow!("创建账号后无法获取"))
}

/// 更新账号
pub async fn update(
    db: &DbConn,
    id: i32,
    req: account::CreateAccountRequest,
) -> Result<account::Model> {
    let account = get_by_id(db, id)
        .await?
        .ok_or_else(|| anyhow!("账号不存在"))?;

    // 如果邮箱地址变更，检查新邮箱是否已被使用
    if account.email != req.email {
        if let Some(_) = get_by_email(db, &req.email).await? {
            return Err(anyhow!("该邮箱地址已被使用"));
        }
    }

    // 如果提供了新密码，则加密
    let encrypted_password = if !req.password.is_empty() {
        let encryptor = PasswordEncryptor::new()?;
        Some(encryptor.encrypt(&req.password)?)
    } else {
        None
    };

    let mut account: account::ActiveModel = account.into();

    account.name = Set(req.name);
    account.email = Set(req.email);
    account.provider = Set(req.provider);

    if let Some(imap_host) = req.imap_host {
        account.imap_host = Set(Some(imap_host));
    }
    if let Some(imap_port) = req.imap_port {
        account.imap_port = Set(Some(imap_port));
    }
    if let Some(imap_ssl) = req.imap_ssl {
        account.imap_ssl = Set(Some(imap_ssl));
    }
    if let Some(smtp_host) = req.smtp_host {
        account.smtp_host = Set(Some(smtp_host));
    }
    if let Some(smtp_port) = req.smtp_port {
        account.smtp_port = Set(Some(smtp_port));
    }
    if let Some(smtp_ssl) = req.smtp_ssl {
        account.smtp_ssl = Set(Some(smtp_ssl));
    }
    if let Some(color) = req.color {
        account.color = Set(Some(color));
    }
    if let Some(encrypted) = encrypted_password {
        account.password = Set(encrypted);
    }

    account.update(db)
        .await
        .map_err(|e| anyhow!("更新账号失败: {}", e))?;

    get_by_id(db, id).await?.ok_or_else(|| anyhow!("更新账号后无法获取"))
}

/// 删除账号
pub async fn delete(db: &DbConn, id: i32) -> Result<()> {
    AccountEntity::delete_by_id(id)
        .exec(db)
        .await
        .map_err(|e| anyhow!("删除账号失败: {}", e))?;

    Ok(())
}

/// 更新最后同步时间
pub async fn update_last_sync(db: &DbConn, id: i32) -> Result<()> {
    let account = get_by_id(db, id)
        .await?
        .ok_or_else(|| anyhow!("账号不存在"))?;

    let mut account: account::ActiveModel = account.into();
    account.last_sync_at = Set(Some(chrono::Utc::now().timestamp()));

    account.update(db)
        .await
        .map_err(|e| anyhow!("更新同步时间失败: {}", e))?;

    Ok(())
}

/// 测试账号连接
pub async fn test_connection(
    db: &DbConn,
    req: &account::CreateAccountRequest,
) -> Result<bool> {
    // TODO: 实现 IMAP 连接测试
    // 这里暂时返回 true，后续实现 IMAP 服务时需要真正测试连接
    Ok(true)
}
