use sea_orm::*;
use anyhow::{anyhow, Result};
use crate::models::{account, AccountEntity};
use crate::crypto::SecureVault;
use std::sync::Arc;
use tokio::sync::Mutex;

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
    AccountEntity::find_by_id(id)
        .one(db)
        .await
        .map_err(|e| anyhow!("获取账号失败: {}", e))
}

/// 根据邮箱获取账号
pub async fn get_by_email(db: &DbConn, email: &str) -> Result<Option<account::Model>> {
    AccountEntity::find()
        .filter(account::Column::Email.eq(email))
        .one(db)
        .await
        .map_err(|e| anyhow!("获取账号失败: {}", e))
}

/// 创建新账号
pub async fn create(
    db: &DbConn,
    vault: &Arc<Mutex<SecureVault>>,
    req: account::CreateAccountRequest,
) -> Result<account::Model> {
    // 检查邮箱是否已存在
    if let Some(_) = get_by_email(db, &req.email).await? {
        return Err(anyhow!("该邮箱地址已存在"));
    }

    // 如果是预设服务商，填充默认配置
    let (imap_config, smtp_config) = account::get_provider_defaults(&req.provider)
        .unwrap_or_else(|| (
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
        ));

    let now = chrono::Utc::now().timestamp();

    // 数据库中不存储密码和 token，设置为空字符串
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
        color: Set(req.color),
        created_at: Set(now),
        updated_at: Set(now),
        auth_type: Set(req.auth_type.clone().unwrap_or("password".to_string())),
        oauth_provider: Set(req.oauth_provider),
        oauth_expires_at: Set(req.oauth_expires_at),
        ..Default::default()
    };

    let result = AccountEntity::insert(new_account)
        .exec(db)
        .await
        .map_err(|e| anyhow!("创建账号失败: {}", e))?;

    let account_id = result.last_insert_id;
    let vault = vault.lock().await;

    // 将密码存储到 Stronghold
    if !req.password.is_empty() {
        let key = format!("password_{}", account_id);
        tracing::info!("存储密码到 Stronghold: account_id={}, key={}", account_id, key);
        vault.store_password(&key, &req.password).await
            .map_err(|e| anyhow!("存储密码到 Stronghold 失败: {}", e))?;

        // 验证密码是否正确存储
        let stored = vault.get_password(&key).await;
        tracing::info!("验证密码存储: key={}, result={:?}", key, stored);
    }

    // 如果有 OAuth Token，存储到 Stronghold
    if let (Some(token), Some(refresh_token), Some(expires_at)) = (
        &req.oauth_token,
        &req.oauth_refresh_token,
        req.oauth_expires_at,
    ) {
        let oauth_token = crate::crypto::OAuthToken {
            access_token: token.clone(),
            refresh_token: refresh_token.clone(),
            expires_at,
        };
        vault.store_token(account_id, &oauth_token).await
            .map_err(|e| anyhow!("存储 OAuth Token 到 Stronghold 失败: {}", e))?;
    }

    drop(vault);

    get_by_id(db, account_id).await?.ok_or_else(|| anyhow!("创建账号后无法获取"))
}

/// 更新账号
pub async fn update(
    db: &DbConn,
    vault: &Arc<Mutex<SecureVault>>,
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

    // OAuth 相关字段更新
    if let Some(auth_type) = req.auth_type {
        account.auth_type = Set(auth_type);
    }
    if let Some(oauth_provider) = req.oauth_provider {
        account.oauth_provider = Set(Some(oauth_provider));
    }
    if let Some(expires_at) = req.oauth_expires_at {
        account.oauth_expires_at = Set(Some(expires_at));
    }

    account.updated_at = Set(chrono::Utc::now().timestamp());

    account.update(db)
        .await
        .map_err(|e| anyhow!("更新账号失败: {}", e))?;

    // 更新 Stronghold 中的敏感信息
    let vault = vault.lock().await;

    // 如果提供了新密码，更新到 Stronghold
    if !req.password.is_empty() {
        let key = format!("password_{}", id);
        vault.store_password(&key, &req.password).await
            .map_err(|e| anyhow!("更新密码到 Stronghold 失败: {}", e))?;
    }

    // 如果有 OAuth Token，更新到 Stronghold
    if let (Some(token), Some(refresh_token), Some(expires_at)) = (
        &req.oauth_token,
        &req.oauth_refresh_token,
        req.oauth_expires_at,
    ) {
        let oauth_token = crate::crypto::OAuthToken {
            access_token: token.clone(),
            refresh_token: refresh_token.clone(),
            expires_at,
        };
        vault.store_token(id, &oauth_token).await
            .map_err(|e| anyhow!("更新 OAuth Token 到 Stronghold 失败: {}", e))?;
    }

    drop(vault);

    get_by_id(db, id).await?.ok_or_else(|| anyhow!("更新账号后无法获取"))
}

/// 删除账号
pub async fn delete(
    db: &DbConn,
    vault: &Arc<Mutex<SecureVault>>,
    id: i32,
) -> Result<()> {
    // 从 Stronghold 删除密码和 token
    let vault = vault.lock().await;
    let password_key = format!("password_{}", id);
    let _ = vault.remove_password(&password_key).await;
    let _ = vault.remove_token(id).await;
    drop(vault);

    // 从数据库删除账号
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
    _db: &DbConn,
    _vault: &Arc<Mutex<SecureVault>>,
    _req: &account::CreateAccountRequest,
) -> Result<bool> {
    // TODO: 实现 IMAP 连接测试
    // 这里暂时返回 true，后续实现 IMAP 服务时需要真正测试连接
    Ok(true)
}

/// 从 Stronghold 获取账号密码
pub async fn get_account_password(
    vault: &Arc<Mutex<SecureVault>>,
    account_id: i32,
) -> Result<String> {
    let vault = vault.lock().await;
    let key = format!("password_{}", account_id);

    tracing::info!("从 Stronghold 获取密码: account_id={}, key={}", account_id, key);

    let password = vault.get_password(&key).await?
        .ok_or_else(|| anyhow!("账号密码不存在（key={}）", key))?;

    tracing::info!("成功获取密码: account_id={}, password_len={}", account_id, password.len());

    Ok(password)
}

/// 从 Stronghold 获取 OAuth Token
pub async fn get_account_oauth_token(
    vault: &Arc<Mutex<SecureVault>>,
    account_id: i32,
) -> Result<crate::crypto::OAuthToken> {
    let vault = vault.lock().await;

    vault.get_token(account_id).await?
        .ok_or_else(|| anyhow!("OAuth Token 不存在"))
}
