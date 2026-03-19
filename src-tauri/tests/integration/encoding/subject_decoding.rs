// 测试：手动解码RFC 2047编码的Subject
//
// 运行：cargo test --test test_decode_subject -- --nocapture

#[cfg(test)]
mod tests {
    use base64::Engine;

    #[test]
    fn test_manual_decode() {
        let encoded = "ob7Sxravt6LGsaG/xPq1xLXn19O3osax0tHLzbTvo6y+tMfrsunUxKOh";

        println!("========================================");
        println!("手动解码 RFC 2047 GBK Base64");
        println!("========================================");
        println!("编码字符串: {}", encoded);

        // 解码Base64
        let engine = base64::engine::general_purpose::STANDARD;
        match engine.decode(encoded) {
            Ok(bytes) => {
                println!("Base64解码后的字节: {:?}", bytes);

                // 使用GBK解码
                let (text, _, _) = encoding_rs::GBK.decode(&bytes);
                println!("GBK解码后的文本: {}", text);
                println!("========================================");

                // 检查是否包含预期的关键词
                if text.contains("移动") || text.contains("发票") {
                    println!("✅ 解码成功! 主题包含预期的中文关键词");
                } else {
                    println!("⚠️  解码后的文本不包含预期的关键词");
                }
            }
            Err(e) => {
                println!("❌ Base64解码失败: {}", e);
            }
        }
    }

    #[test]
    fn test_parser_decode() {
        let raw_header = "Subject: =?gbk?b?ob7Sxravt6LGsaG/xPq1xLXn19O3osax0tHLzbTvo6y+tMfrsunUxKOh?=";

        println!("========================================");
        println!("使用mail_parser解码");
        println!("========================================");

        use mail_parser::MessageParser;

        // 构造一个简单的邮件头
        let email = format!("{}\r\n\r\n", raw_header);
        let message = MessageParser::default().parse(email.as_bytes());

        if let Some(msg) = message {
            if let Some(subject) = msg.subject() {
                println!("mail_parser解码后的主题: {}", subject);

                // 检查是否包含乱码
                if subject.contains('\u{FFFD}') {
                    println!("⚠️  包含替换字符(U+FFFD)，表明解码不完全");
                }

                // 检查GBK错误模式
                if subject.contains('ƶ') || subject.contains('Ʊ') {
                    println!("⚠️  包含GBK编码错误的典型字符");
                    println!("说明mail_parser可能没有正确处理RFC 2047 GBK编码");
                }
            }
        }

        println!("========================================");
    }
}
