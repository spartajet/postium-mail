// 测试：手动解码RFC 2047编码的Subject
//
// 运行：cargo test --test test_decode_subject -- --nocapture

#[cfg(test)]
mod tests {
    use base64::Engine;

    // 引入测试辅助宏
    use crate::test_macros::*;

    #[test]
    fn test_manual_decode() {
        let encoded = "ob7Sxravt6LGsaG/xPq1xLXn19O3osax0tHLzbTvo6y+tMfrsunUxKOh";

        test_info!("========================================");
        test_info!("手动解码 RFC 2047 GBK Base64");
        test_info!("========================================");
        test_info!("编码字符串: {}", encoded);

        // 解码Base64
        let engine = base64::engine::general_purpose::STANDARD;
        match engine.decode(encoded) {
            Ok(bytes) => {
                test_info!("Base64解码后的字节: {:?}", bytes);

                // 使用GBK解码
                let (text, _, _) = encoding_rs::GBK.decode(&bytes);
                test_info!("GBK解码后的文本: {}", text);
                test_info!("========================================");

                // 检查是否包含预期的关键词
                if text.contains("移动") || text.contains("发票") {
                    test_success!("解码成功! 主题包含预期的中文关键词");
                } else {
                    test_warn!("解码后的文本不包含预期的关键词");
                }
            }
            Err(e) => {
                test_error!("Base64解码失败: {}", e);
            }
        }
    }

    #[test]
    fn test_parser_decode() {
        let raw_header = "Subject: =?gbk?b?ob7Sxravt6LGsaG/xPq1xLXn19O3osax0tHLzbTvo6y+tMfrsunUxKOh?=";

        test_info!("========================================");
        test_info!("使用mail_parser解码");
        test_info!("========================================");

        use mail_parser::MessageParser;

        // 构造一个简单的邮件头
        let email = format!("{}\r\n\r\n", raw_header);
        let message = MessageParser::default().parse(email.as_bytes());

        if let Some(msg) = message {
            if let Some(subject) = msg.subject() {
                test_info!("mail_parser解码后的主题: {}", subject);

                // 检查是否包含乱码
                if subject.contains('\u{FFFD}') {
                    test_warn!("包含替换字符(U+FFFD)，表明解码不完全");
                }

                // 检查GBK错误模式
                if subject.contains('ƶ') || subject.contains('Ʊ') {
                    test_warn!("包含GBK编码错误的典型字符");
                    test_info!("说明mail_parser可能没有正确处理RFC 2047 GBK编码");
                }
            }
        }

        test_info!("========================================");
    }
}
