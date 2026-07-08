# Email Compose M1 Recipients Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 实现写信窗口 To/Cc/Bcc 的 chip 输入、后端权威邮件地址解析，以及 Bcc UI 接入发送和草稿。

**Architecture:** 后端新增 `parse_email_addresses` 命令，使用现有 Rust `mail-parser` 作为权威地址解析器并返回有效、无效、重复地址。前端新增 `RecipientField.svelte` 作为统一收件人字段组件，`ComposeModal.svelte` 只维护 To/Cc/Bcc 三组 chip 状态，并在发送和草稿保存时提交有效地址数组。

**Tech Stack:** Rust 2024, Tauri v2, specta/tauri-specta, mail-parser 0.11, Svelte 5, TypeScript, Vitest, Testing Library.

## Global Constraints

- 文档和用户沟通使用中文。
- 所有 shell 命令使用 `rtk` 前缀。
- 没有明确指令不要提交代码；本计划中的提交步骤仅在用户要求按计划执行并允许提交时执行。
- M1 不引入新的前端 npm 邮件地址解析依赖。
- M1 不改变 `SendEmailRequest` 和 `SaveDraftRequest` 的核心 DTO 形状，提交时仍传 `Vec<String>`。
- M1 不做联系人自动补全、通讯录、回复/全部回复/转发语义、签名、Reply-To 或发件别名。
- 正式发送仍要求至少一个有效收件人、主题非空、正文非空。
- Bcc 地址只进入 SMTP envelope，不写入最终 MIME header。

---

## File Structure

- `src-tauri/src/service/email_address.rs`
  - 新建后端地址解析模块。
  - 定义 `ParsedEmailAddress`、`InvalidEmailAddress`、`DuplicateEmailAddress`、`ParseEmailAddressesResponse`。
  - 暴露 `parse_email_addresses(input: String) -> ParseEmailAddressesResponse`。

- `src-tauri/src/service/mod.rs`
  - 导出 `email_address` 模块。

- `src-tauri/src/service/email_service.rs`
  - re-export 地址解析 DTO。
  - 增加 `EmailService::parse_email_addresses` 方法。

- `src-tauri/src/command/email.rs`
  - 增加 Tauri 命令 `parse_email_addresses`。

- `src-tauri/src/lib.rs`
  - 注册 `command::email::parse_email_addresses` 到 specta command builder。

- `src-tauri/tests/email_commands.rs`
  - 增加地址解析命令测试。
  - 保留并补充 Bcc 不写 header 的回归测试。

- `src/lib/components/email/RecipientField.svelte`
  - 新建统一收件人 chip 输入组件。

- `src/lib/components/email/recipient.ts`
  - 新建前端收件人类型与纯函数辅助：`RecipientChip`、`validEmails`、`hasInvalidRecipients`。

- `src/lib/i18n/zh-CN.ts`
  - 增加无效地址、重复地址、显示 Cc/Bcc、隐藏 Cc/Bcc 文案。

- `src/lib/i18n/en-US.ts`
  - 增加对应英文文案。

- `src/lib/components/email/ComposeModal.svelte`
  - 用 `RecipientField` 替换 To/Cc 文本输入。
  - 增加 Bcc 展开状态和 Bcc 提交。
  - 调整草稿保存和正式发送的收件人派生。

- `src/lib/__tests__/components/RecipientField.test.ts`
  - 新增组件测试。

- `src/lib/__tests__/components/ComposeModal.test.ts`
  - 更新现有测试，覆盖 To/Cc/Bcc chip 集成。

- `src/lib/bindings.ts`
  - 通过当前项目的 specta debug export 更新；若本地环境无法触发导出，再按计划中的类型和命令签名手工同步。

---

### Task 1: 后端地址解析模块和命令

**Files:**
- Create: `src-tauri/src/service/email_address.rs`
- Modify: `src-tauri/src/service/mod.rs`
- Modify: `src-tauri/src/service/email_service.rs`
- Modify: `src-tauri/src/command/email.rs`
- Modify: `src-tauri/src/lib.rs`
- Test: `src-tauri/tests/email_commands.rs`

**Interfaces:**
- Produces:
  - `ParsedEmailAddress { name: Option<String>, email: String, raw: String }`
  - `InvalidEmailAddress { raw: String, reason: String }`
  - `DuplicateEmailAddress { raw: String, email: String }`
  - `ParseEmailAddressesResponse { addresses: Vec<ParsedEmailAddress>, invalid: Vec<InvalidEmailAddress>, duplicates: Vec<DuplicateEmailAddress> }`
  - `EmailService::parse_email_addresses(&self, input: String) -> Result<ParseEmailAddressesResponse, MailError>`
  - command `parse_email_addresses(input: String)`
- Consumes:
  - Existing `mail-parser = "0.11"`
  - Existing command registration pattern in `src-tauri/src/command/email.rs`

- [ ] **Step 1: Add failing tests for backend address parsing**

Add tests to `src-tauri/tests/email_commands.rs`:

```rust
#[tokio::test]
async fn parse_email_addresses_accepts_common_address_forms() {
    let services = TestServices::new().await;

    let result = services
        .email_service
        .parse_email_addresses(
            r#"alice@example.com, Bob <bob@example.com>, "Alice, Inc." <team@example.com>"#
                .to_string(),
        )
        .await
        .unwrap();

    assert_eq!(
        result
            .addresses
            .iter()
            .map(|address| (address.name.as_deref(), address.email.as_str()))
            .collect::<Vec<_>>(),
        vec![
            (None, "alice@example.com"),
            (Some("Bob"), "bob@example.com"),
            (Some("Alice, Inc."), "team@example.com"),
        ]
    );
    assert!(result.invalid.is_empty());
    assert!(result.duplicates.is_empty());
}

#[tokio::test]
async fn parse_email_addresses_reports_invalid_and_duplicates() {
    let services = TestServices::new().await;

    let result = services
        .email_service
        .parse_email_addresses(
            "alice@example.com, not-an-address, Alice <ALICE@example.com>".to_string(),
        )
        .await
        .unwrap();

    assert_eq!(result.addresses.len(), 1);
    assert_eq!(result.addresses[0].email, "alice@example.com");
    assert_eq!(result.invalid.len(), 1);
    assert_eq!(result.invalid[0].raw, "not-an-address");
    assert_eq!(result.duplicates.len(), 1);
    assert_eq!(result.duplicates[0].email, "ALICE@example.com");
}

#[tokio::test]
async fn parse_email_addresses_extracts_group_members() {
    let services = TestServices::new().await;

    let result = services
        .email_service
        .parse_email_addresses("Friends: Alice <alice@example.com>, bob@example.com;".to_string())
        .await
        .unwrap();

    assert_eq!(
        result
            .addresses
            .iter()
            .map(|address| address.email.as_str())
            .collect::<Vec<_>>(),
        vec!["alice@example.com", "bob@example.com"]
    );
    assert!(result.invalid.is_empty());
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test email_commands parse_email_addresses -- --nocapture
```

Expected: fail because `EmailService::parse_email_addresses` and related types do not exist.

- [ ] **Step 3: Implement `email_address.rs` DTOs and parser**

Create `src-tauri/src/service/email_address.rs`:

```rust
use crate::error::MailError;
use mail_parser::{Addr, MessageParser};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::collections::HashSet;

#[derive(Debug, Clone, Serialize, Deserialize, Type, PartialEq, Eq)]
pub struct ParsedEmailAddress {
    pub name: Option<String>,
    pub email: String,
    pub raw: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type, PartialEq, Eq)]
pub struct InvalidEmailAddress {
    pub raw: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type, PartialEq, Eq)]
pub struct DuplicateEmailAddress {
    pub raw: String,
    pub email: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type, PartialEq, Eq)]
pub struct ParseEmailAddressesResponse {
    pub addresses: Vec<ParsedEmailAddress>,
    pub invalid: Vec<InvalidEmailAddress>,
    pub duplicates: Vec<DuplicateEmailAddress>,
}

pub fn parse_email_addresses(input: String) -> Result<ParseEmailAddressesResponse, MailError> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Ok(ParseEmailAddressesResponse {
            addresses: Vec::new(),
            invalid: Vec::new(),
            duplicates: Vec::new(),
        });
    }

    let raw = format!("To: {trimmed}\r\n\r\n");
    let Some(message) = MessageParser::default().parse(raw.as_bytes()) else {
        return Ok(ParseEmailAddressesResponse {
            addresses: Vec::new(),
            invalid: vec![InvalidEmailAddress {
                raw: trimmed.to_string(),
                reason: "无法解析邮件地址".to_string(),
            }],
            duplicates: Vec::new(),
        });
    };

    let mut seen = HashSet::new();
    let mut response = ParseEmailAddressesResponse {
        addresses: Vec::new(),
        invalid: Vec::new(),
        duplicates: Vec::new(),
    };

    if let Some(addresses) = message.to() {
        collect_mail_parser_addresses(addresses, &mut seen, &mut response);
    }

    if response.addresses.is_empty() && response.duplicates.is_empty() {
        response.invalid.push(InvalidEmailAddress {
            raw: trimmed.to_string(),
            reason: "无法解析邮件地址".to_string(),
        });
    }

    Ok(response)
}

fn collect_mail_parser_addresses(
    address: &mail_parser::Address<'_>,
    seen: &mut HashSet<String>,
    response: &mut ParseEmailAddressesResponse,
) {
    match address {
        mail_parser::Address::List(addresses) => {
            for addr in addresses {
                collect_addr(addr, seen, response);
            }
        }
        mail_parser::Address::Group(groups) => {
            for group in groups {
                for addr in &group.addresses {
                    collect_addr(addr, seen, response);
                }
            }
        }
    }
}

fn collect_addr(
    addr: &Addr<'_>,
    seen: &mut HashSet<String>,
    response: &mut ParseEmailAddressesResponse,
) {
    let Some(address) = addr.address.as_ref() else {
        return;
    };
    let email = address.to_string();
    let dedupe_key = email.to_ascii_lowercase();
    let name = addr.name.as_ref().map(|value| value.to_string());
    let raw = match name.as_deref() {
        Some(name) if !name.trim().is_empty() => format!("{name} <{email}>"),
        _ => email.clone(),
    };

    if seen.insert(dedupe_key) {
        response.addresses.push(ParsedEmailAddress { name, email, raw });
    } else {
        response.duplicates.push(DuplicateEmailAddress { raw, email });
    }
}
```

- [ ] **Step 4: Export service API and command**

In `src-tauri/src/service/mod.rs`, add:

```rust
pub mod email_address;
```

In `src-tauri/src/service/email_service.rs`, add exports near other service DTO exports:

```rust
pub use crate::service::email_address::{
    DuplicateEmailAddress, InvalidEmailAddress, ParseEmailAddressesResponse, ParsedEmailAddress,
};
```

Add method inside `impl EmailService`:

```rust
pub async fn parse_email_addresses(
    &self,
    input: String,
) -> Result<ParseEmailAddressesResponse, MailError> {
    crate::service::email_address::parse_email_addresses(input)
}
```

In `src-tauri/src/command/email.rs`, import `ParseEmailAddressesResponse` from `email_service` and add:

```rust
#[tauri::command]
#[specta::specta]
pub async fn parse_email_addresses(
    service: tauri::State<'_, crate::service::email_service::EmailService>,
    input: String,
) -> Result<ParseEmailAddressesResponse, MailError> {
    service.parse_email_addresses(input).await
}
```

In `src-tauri/src/lib.rs`, register the command in `collect_commands!`:

```rust
command::email::parse_email_addresses,
```

- [ ] **Step 5: Run backend parser tests**

Run:

```bash
rtk cargo test --manifest-path src-tauri/Cargo.toml --test email_commands parse_email_addresses -- --nocapture
```

Expected: pass.

- [ ] **Step 6: Commit backend parser task if commits are allowed**

Only if the user has explicitly allowed commits for implementation tasks:

```bash
rtk git add src-tauri/src/service/email_address.rs src-tauri/src/service/mod.rs src-tauri/src/service/email_service.rs src-tauri/src/command/email.rs src-tauri/src/lib.rs src-tauri/tests/email_commands.rs
rtk git commit -m "接入发件地址解析命令"
```

---

### Task 2: 更新绑定和前端收件人辅助类型

**Files:**
- Modify: `src/lib/bindings.ts`
- Create: `src/lib/components/email/recipient.ts`
- Test: `src/lib/__tests__/components/RecipientField.test.ts`

**Interfaces:**
- Consumes:
  - command `parse_email_addresses(input: string)`
  - `ParseEmailAddressesResponse`
- Produces:
  - `RecipientChip`
  - `recipientLabel(chip: RecipientChip): string`
  - `validEmails(chips: RecipientChip[]): string[]`
  - `hasInvalidRecipients(chips: RecipientChip[]): boolean`
  - `chipsFromParsedResponse(response: ParseEmailAddressesResponse): RecipientChip[]`

- [ ] **Step 1: Add failing unit tests for recipient helpers**

Create `src/lib/__tests__/components/RecipientField.test.ts` with helper tests first:

```ts
import { describe, expect, it } from "vitest";
import {
    chipsFromParsedResponse,
    hasInvalidRecipients,
    recipientLabel,
    validEmails,
} from "$lib/components/email/recipient";

describe("recipient helpers", () => {
    it("converts parsed response into valid and invalid chips", () => {
        const chips = chipsFromParsedResponse({
            addresses: [
                { name: "Alice", email: "alice@example.com", raw: "Alice <alice@example.com>" },
            ],
            invalid: [{ raw: "bad", reason: "无法解析邮件地址" }],
            duplicates: [{ raw: "ALICE@example.com", email: "ALICE@example.com" }],
        });

        expect(chips).toHaveLength(2);
        expect(chips[0]).toMatchObject({
            name: "Alice",
            email: "alice@example.com",
            raw: "Alice <alice@example.com>",
            valid: true,
        });
        expect(chips[1]).toMatchObject({
            raw: "bad",
            valid: false,
            reason: "无法解析邮件地址",
        });
    });

    it("returns only valid emails and detects invalid chips", () => {
        const chips = [
            { id: "1", raw: "Alice <alice@example.com>", email: "alice@example.com", valid: true },
            { id: "2", raw: "bad", valid: false, reason: "bad" },
        ];

        expect(validEmails(chips)).toEqual(["alice@example.com"]);
        expect(hasInvalidRecipients(chips)).toBe(true);
        expect(recipientLabel(chips[0])).toBe("Alice <alice@example.com>");
        expect(recipientLabel(chips[1])).toBe("bad");
    });
});
```

- [ ] **Step 2: Run frontend test to verify it fails**

Run:

```bash
rtk bun node_modules/vitest/vitest.mjs run src/lib/__tests__/components/RecipientField.test.ts
```

Expected: fail because `recipient.ts` does not exist.

- [ ] **Step 3: Update `src/lib/bindings.ts`**

Run:

```bash
rtk cargo check --manifest-path src-tauri/Cargo.toml
```

Expected: Rust check passes and the debug specta export updates `src/lib/bindings.ts`.

Verify:

```bash
rtk rg -n "ParseEmailAddressesResponse|ParsedEmailAddress|parseEmailAddresses" src/lib/bindings.ts
```

Expected: all three names are present.

If `cargo check` passes but the binding file does not contain the new command in this environment, manually add these exported types following the existing generated style:

```ts
export type ParsedEmailAddress = {
    name: string | null;
    email: string;
    raw: string;
};

export type InvalidEmailAddress = {
    raw: string;
    reason: string;
};

export type DuplicateEmailAddress = {
    raw: string;
    email: string;
};

export type ParseEmailAddressesResponse = {
    addresses: ParsedEmailAddress[];
    invalid: InvalidEmailAddress[];
    duplicates: DuplicateEmailAddress[];
};
```

Add command wrapper:

```ts
parseEmailAddresses: (input: string) =>
    invoke<Result<ParseEmailAddressesResponse, MailError>>("parse_email_addresses", { input }),
```

Match exact casing and return wrapper used by existing `describeLocalAttachments`, `saveDraft`, and `sendEmail` entries.

- [ ] **Step 4: Implement `recipient.ts` helpers**

Create `src/lib/components/email/recipient.ts`:

```ts
import type { ParseEmailAddressesResponse } from "$lib/bindings";

export type RecipientChip = {
    id: string;
    name?: string;
    email?: string;
    raw: string;
    valid: boolean;
    reason?: string;
};

let nextRecipientChipId = 0;

export function createRecipientChipId() {
    nextRecipientChipId += 1;
    return `recipient-${nextRecipientChipId}`;
}

export function chipsFromParsedResponse(
    response: ParseEmailAddressesResponse,
): RecipientChip[] {
    return [
        ...response.addresses.map((address) => ({
            id: createRecipientChipId(),
            name: address.name ?? undefined,
            email: address.email,
            raw: address.raw,
            valid: true,
        })),
        ...response.invalid.map((invalid) => ({
            id: createRecipientChipId(),
            raw: invalid.raw,
            valid: false,
            reason: invalid.reason,
        })),
    ];
}

export function recipientLabel(chip: RecipientChip) {
    if (chip.valid && chip.email) {
        return chip.name ? `${chip.name} <${chip.email}>` : chip.email;
    }
    return chip.raw;
}

export function validEmails(chips: RecipientChip[]) {
    return chips
        .filter((chip) => chip.valid && Boolean(chip.email))
        .map((chip) => chip.email as string);
}

export function hasInvalidRecipients(chips: RecipientChip[]) {
    return chips.some((chip) => !chip.valid);
}

export function mergeRecipientChips(
    existing: RecipientChip[],
    incoming: RecipientChip[],
) {
    const seen = new Set(
        existing
            .filter((chip) => chip.valid && chip.email)
            .map((chip) => chip.email!.toLowerCase()),
    );
    const merged = [...existing];
    for (const chip of incoming) {
        if (chip.valid && chip.email) {
            const key = chip.email.toLowerCase();
            if (seen.has(key)) continue;
            seen.add(key);
        }
        merged.push(chip);
    }
    return merged;
}
```

- [ ] **Step 5: Run helper tests**

Run:

```bash
rtk bun node_modules/vitest/vitest.mjs run src/lib/__tests__/components/RecipientField.test.ts
```

Expected: helper tests pass.

- [ ] **Step 6: Commit helper task if commits are allowed**

Only if the user has explicitly allowed commits for implementation tasks:

```bash
rtk git add src/lib/bindings.ts src/lib/components/email/recipient.ts src/lib/__tests__/components/RecipientField.test.ts
rtk git commit -m "添加收件人 chip 辅助类型"
```

---

### Task 3: RecipientField 组件

**Files:**
- Create: `src/lib/components/email/RecipientField.svelte`
- Modify: `src/lib/__tests__/components/RecipientField.test.ts`

**Interfaces:**
- Consumes:
  - `commands.parseEmailAddresses(input: string)`
  - `RecipientChip`
  - `chipsFromParsedResponse`
  - `mergeRecipientChips`
  - `recipientLabel`
- Produces:
  - Svelte component props:
    - `label: string`
    - `chips: RecipientChip[]`
    - `placeholder?: string`
    - `onChange: (chips: RecipientChip[]) => void`
    - `testIdPrefix: string`

- [ ] **Step 1: Add failing component tests**

Append to `src/lib/__tests__/components/RecipientField.test.ts`:

```ts
import { fireEvent, render, screen, waitFor } from "@testing-library/svelte";
import RecipientField from "$lib/components/email/RecipientField.svelte";
import { mockInvoke } from "../mocks/tauri";

describe("RecipientField component", () => {
    it("parses input on Enter and renders a chip", async () => {
        const onChange = vi.fn();
        mockInvoke.mockResolvedValue({
            addresses: [{ name: "Alice", email: "alice@example.com", raw: "Alice <alice@example.com>" }],
            invalid: [],
            duplicates: [],
        });

        render(RecipientField, {
            props: {
                label: "收件人",
                chips: [],
                testIdPrefix: "to",
                onChange,
            },
        });

        const input = screen.getByTestId("to-recipient-input");
        await fireEvent.input(input, { target: { value: "Alice <alice@example.com>" } });
        await fireEvent.keyDown(input, { key: "Enter" });

        await waitFor(() => {
            expect(onChange).toHaveBeenCalledWith([
                expect.objectContaining({
                    name: "Alice",
                    email: "alice@example.com",
                    valid: true,
                }),
            ]);
        });
        expect(mockInvoke).toHaveBeenCalledWith("parse_email_addresses", {
            input: "Alice <alice@example.com>",
        });
    });

    it("renders invalid chips and lets the user delete chips", async () => {
        const onChange = vi.fn();
        render(RecipientField, {
            props: {
                label: "收件人",
                testIdPrefix: "to",
                chips: [
                    { id: "1", raw: "bad", valid: false, reason: "无法解析邮件地址" },
                ],
                onChange,
            },
        });

        expect(screen.getByText("bad")).toBeTruthy();
        await fireEvent.click(screen.getByTestId("to-recipient-remove-1"));

        expect(onChange).toHaveBeenCalledWith([]);
    });
});
```

Ensure this file imports `vi` from `vitest` once at top. If Task 2 already has `describe`, `expect`, `it`, extend that import to include `vi`.

- [ ] **Step 2: Run component tests to verify they fail**

Run:

```bash
rtk bun node_modules/vitest/vitest.mjs run src/lib/__tests__/components/RecipientField.test.ts
```

Expected: fail because `RecipientField.svelte` does not exist.

- [ ] **Step 3: Implement `RecipientField.svelte`**

Create `src/lib/components/email/RecipientField.svelte`:

```svelte
<script lang="ts">
    import { X } from "lucide-svelte";
    import { commands } from "$lib/bindings";
    import {
        chipsFromParsedResponse,
        mergeRecipientChips,
        recipientLabel,
        type RecipientChip,
    } from "./recipient";

    type Props = {
        label: string;
        chips: RecipientChip[];
        placeholder?: string;
        testIdPrefix: string;
        onChange: (chips: RecipientChip[]) => void;
    };

    let {
        label,
        chips,
        placeholder = "",
        testIdPrefix,
        onChange,
    }: Props = $props();

    let inputValue = $state("");
    let parsing = $state(false);
    let parseError = $state("");

    async function commitInput() {
        const value = inputValue.trim();
        if (!value || parsing) return;

        parsing = true;
        parseError = "";
        try {
            const result = await commands.parseEmailAddresses(value);
            if (result.status === "error") {
                parseError = result.error.message as string;
                return;
            }
            const next = chipsFromParsedResponse(result.data);
            onChange(mergeRecipientChips(chips, next));
            inputValue = "";
        } catch (error) {
            parseError = error instanceof Error ? error.message : String(error);
        } finally {
            parsing = false;
        }
    }

    function removeChip(id: string) {
        onChange(chips.filter((chip) => chip.id !== id));
    }

    async function handleKeydown(event: KeyboardEvent) {
        if (["Enter", "Tab", ",", ";"].includes(event.key)) {
            if (inputValue.trim()) {
                event.preventDefault();
                await commitInput();
            }
        } else if (event.key === "Backspace" && !inputValue && chips.length > 0) {
            event.preventDefault();
            onChange(chips.slice(0, -1));
        }
    }

    async function handlePaste(event: ClipboardEvent) {
        const text = event.clipboardData?.getData("text") ?? "";
        if (!text.trim()) return;
        event.preventDefault();
        inputValue = text;
        await commitInput();
    }
</script>

<div class="flex items-start border-b border-border px-4">
    <span class="w-14 shrink-0 pt-2 text-sm text-muted-foreground">{label}</span>
    <div class="min-w-0 flex-1 py-1.5">
        <div
            class="flex min-h-8 flex-wrap items-center gap-1.5"
            data-testid={`${testIdPrefix}-recipient-field`}
        >
            {#each chips as chip (chip.id)}
                <span
                    class="inline-flex max-w-full items-center gap-1 rounded-md border px-2 py-1 text-xs {chip.valid
                        ? 'border-border bg-glass text-foreground'
                        : 'border-destructive/50 bg-destructive/10 text-destructive'}"
                    title={chip.reason || recipientLabel(chip)}
                >
                    <span class="truncate">{recipientLabel(chip)}</span>
                    <button
                        type="button"
                        data-testid={`${testIdPrefix}-recipient-remove-${chip.id}`}
                        class="shrink-0 rounded-sm text-muted-foreground hover:text-foreground"
                        aria-label="Remove recipient"
                        onclick={() => removeChip(chip.id)}
                    >
                        <X size={12} />
                    </button>
                </span>
            {/each}

            <input
                data-testid={`${testIdPrefix}-recipient-input`}
                class="min-w-32 flex-1 bg-transparent text-sm text-foreground outline-none placeholder:text-muted-foreground"
                bind:value={inputValue}
                {placeholder}
                disabled={parsing}
                onkeydown={handleKeydown}
                onpaste={handlePaste}
                onblur={() => void commitInput()}
            />
        </div>
        {#if parseError}
            <p class="mt-1 text-xs text-destructive">{parseError}</p>
        {/if}
    </div>
</div>
```

- [ ] **Step 4: Run RecipientField tests**

Run:

```bash
rtk bun node_modules/vitest/vitest.mjs run src/lib/__tests__/components/RecipientField.test.ts
```

Expected: tests pass.

- [ ] **Step 5: Commit RecipientField task if commits are allowed**

Only if the user has explicitly allowed commits for implementation tasks:

```bash
rtk git add src/lib/components/email/RecipientField.svelte src/lib/__tests__/components/RecipientField.test.ts
rtk git commit -m "添加收件人 chip 输入组件"
```

---

### Task 4: ComposeModal 接入 To/Cc/Bcc chip 输入

**Files:**
- Modify: `src/lib/components/email/ComposeModal.svelte`
- Modify: `src/lib/i18n/zh-CN.ts`
- Modify: `src/lib/i18n/en-US.ts`
- Modify: `src/lib/__tests__/components/ComposeModal.test.ts`

**Interfaces:**
- Consumes:
  - `RecipientField.svelte`
  - `RecipientChip`
  - `validEmails(chips)`
  - `hasInvalidRecipients(chips)`
- Produces:
  - Compose sends `bcc` from valid Bcc chips.
  - Compose saves draft `bcc` from valid Bcc chips.
  - Compose shows error when invalid chips exist.

- [ ] **Step 1: Update ComposeModal tests to fail for new behavior**

Modify `src/lib/__tests__/components/ComposeModal.test.ts`:

Add `bcc` and invalid recipient texts to i18n mock:

```ts
bcc: "密送",
showCc: "抄送",
showBcc: "密送",
invalidRecipients: "请修正或删除无效收件人",
```

Mock parser command in `beforeEach`:

```ts
mockInvoke.mockImplementation((cmd, args) => {
    if (cmd === "parse_email_addresses") {
        const input = String(args?.input ?? "");
        if (input === "bad") {
            return Promise.resolve({
                addresses: [],
                invalid: [{ raw: "bad", reason: "无法解析邮件地址" }],
                duplicates: [],
            });
        }
        return Promise.resolve({
            addresses: [{ name: null, email: input, raw: input }],
            invalid: [],
            duplicates: [],
        });
    }
    return Promise.resolve({
        message_id: "<message-id@example.com>",
        local_email_id: 1,
        remote_archived: true,
        remote_archive_error: null,
    });
});
```

Replace direct `compose-to-input` usage in new tests with recipient field behavior:

```ts
async function addRecipient(testIdPrefix: string, value: string) {
    const input = await screen.findByTestId(`${testIdPrefix}-recipient-input`);
    await fireEvent.input(input, { target: { value } });
    await fireEvent.keyDown(input, { key: "Enter" });
}
```

Add tests:

```ts
it("展开 Bcc 后发送请求包含密送地址", async () => {
    const { component } = render(ComposeModal);

    component.show();
    await addRecipient("to", "to@example.com");
    await fireEvent.click(screen.getByTestId("compose-show-bcc-button"));
    await addRecipient("bcc", "hidden@example.com");
    await fireEvent.input(screen.getByTestId("compose-subject-input"), {
        target: { value: "Hello" },
    });
    await fillBody("Body");
    await fireEvent.click(screen.getByTestId("compose-send-button"));

    expect(mockInvoke).toHaveBeenCalledWith("send_email", {
        request: expect.objectContaining({
            to: ["to@example.com"],
            bcc: ["hidden@example.com"],
        }),
    });
});

it("存在无效收件人 chip 时阻止发送", async () => {
    const { component } = render(ComposeModal);

    component.show();
    await addRecipient("to", "bad");
    await fireEvent.input(screen.getByTestId("compose-subject-input"), {
        target: { value: "Hello" },
    });
    await fillBody("Body");
    await fireEvent.click(screen.getByTestId("compose-send-button"));

    expect(await screen.findByText("请修正或删除无效收件人")).toBeTruthy();
    expect(mockInvoke).not.toHaveBeenCalledWith("send_email", expect.anything());
});

it("草稿保存包含有效 Bcc 地址", async () => {
    vi.useFakeTimers();
    const { component } = render(ComposeModal);

    component.show();
    await fireEvent.click(await screen.findByTestId("compose-show-bcc-button"));
    await addRecipient("bcc", "hidden@example.com");

    await vi.runOnlyPendingTimersAsync();

    expect(mockInvoke).toHaveBeenCalledWith("save_draft", {
        request: expect.objectContaining({
            bcc: ["hidden@example.com"],
        }),
    });
});
```

Existing tests that use `compose-to-input` must be updated to use `addRecipient("to", "to@example.com")`.

- [ ] **Step 2: Run ComposeModal tests to verify they fail**

Run:

```bash
rtk bun node_modules/vitest/vitest.mjs run src/lib/__tests__/components/ComposeModal.test.ts
```

Expected: fail because ComposeModal still uses text inputs and no Bcc UI.

- [ ] **Step 3: Add i18n copy**

In `src/lib/i18n/zh-CN.ts` email section add:

```ts
showCc: "抄送",
showBcc: "密送",
hideCc: "隐藏抄送",
hideBcc: "隐藏密送",
invalidRecipients: "请修正或删除无效收件人",
```

In `src/lib/i18n/en-US.ts` email section add:

```ts
showCc: "CC",
showBcc: "BCC",
hideCc: "Hide CC",
hideBcc: "Hide BCC",
invalidRecipients: "Fix or remove invalid recipients",
```

- [ ] **Step 4: Refactor ComposeModal state and helpers**

In `src/lib/components/email/ComposeModal.svelte`, import:

```ts
import RecipientField from "./RecipientField.svelte";
import {
    hasInvalidRecipients,
    validEmails,
    type RecipientChip,
} from "./recipient";
```

Replace string states:

```ts
let toRecipients = $state<RecipientChip[]>([]);
let ccRecipients = $state<RecipientChip[]>([]);
let bccRecipients = $state<RecipientChip[]>([]);
let showCcField = $state(false);
let showBccField = $state(false);
```

Add helpers:

```ts
function allRecipientChips() {
    return [...toRecipients, ...ccRecipients, ...bccRecipients];
}

function hasRecipientErrors() {
    return hasInvalidRecipients(toRecipients)
        || hasInvalidRecipients(ccRecipients)
        || hasInvalidRecipients(bccRecipients);
}

function recipientRequestParts() {
    return {
        to: validEmails(toRecipients),
        cc: validEmails(ccRecipients),
        bcc: validEmails(bccRecipients),
    };
}
```

Update `hasDraftContent()`:

```ts
return Boolean(
    allRecipientChips().length > 0 ||
        subject.trim() ||
        (richEditor?.getText() || "").trim() ||
        attachments.length > 0,
);
```

Update `close()` to clear recipient arrays and collapse Cc/Bcc:

```ts
toRecipients = [];
ccRecipients = [];
bccRecipients = [];
showCcField = false;
showBccField = false;
```

Remove `parseRecipients` if no longer used.

- [ ] **Step 5: Update save draft and send flows**

In `persistDraftNow`, derive recipients:

```ts
const recipients = recipientRequestParts();
```

Pass:

```ts
to: recipients.to,
cc: recipients.cc,
bcc: recipients.bcc,
```

In `handleSend`, derive recipients:

```ts
const recipients = recipientRequestParts();
if (hasRecipientErrors()) {
    error = t.email.invalidRecipients;
    return;
}
if (
    recipients.to.length === 0 &&
    recipients.cc.length === 0 &&
    recipients.bcc.length === 0
) {
    error = t.email.sendRequiresRecipient;
    return;
}
```

Pass `recipients.to`, `recipients.cc`, `recipients.bcc` to `commands.sendEmail`.

- [ ] **Step 6: Replace recipient field markup**

In ComposeModal field area, replace To/Cc rows with:

```svelte
<div class="border-b border-border">
    <div class="relative">
        <RecipientField
            label={t.email.to}
            chips={toRecipients}
            testIdPrefix="to"
            onChange={(chips) => {
                toRecipients = chips;
                scheduleDraftSave();
            }}
        />
        <div class="absolute right-4 top-2 flex gap-2">
            {#if !showCcField && ccRecipients.length === 0}
                <button
                    type="button"
                    data-testid="compose-show-cc-button"
                    class="text-xs text-muted-foreground hover:text-foreground"
                    onclick={() => (showCcField = true)}
                >
                    {t.email.showCc}
                </button>
            {/if}
            {#if !showBccField && bccRecipients.length === 0}
                <button
                    type="button"
                    data-testid="compose-show-bcc-button"
                    class="text-xs text-muted-foreground hover:text-foreground"
                    onclick={() => (showBccField = true)}
                >
                    {t.email.showBcc}
                </button>
            {/if}
        </div>
    </div>

    {#if showCcField || ccRecipients.length > 0}
        <RecipientField
            label={t.email.cc}
            chips={ccRecipients}
            testIdPrefix="cc"
            onChange={(chips) => {
                ccRecipients = chips;
                scheduleDraftSave();
            }}
        />
    {/if}

    {#if showBccField || bccRecipients.length > 0}
        <RecipientField
            label={t.email.bcc}
            chips={bccRecipients}
            testIdPrefix="bcc"
            onChange={(chips) => {
                bccRecipients = chips;
                scheduleDraftSave();
            }}
        />
    {/if}
```

Keep the existing subject row after these fields. Preserve existing styling conventions and avoid nested card styling.

- [ ] **Step 7: Run ComposeModal tests**

Run:

```bash
rtk bun node_modules/vitest/vitest.mjs run src/lib/__tests__/components/ComposeModal.test.ts
```

Expected: pass.

- [ ] **Step 8: Commit Compose integration task if commits are allowed**

Only if the user has explicitly allowed commits for implementation tasks:

```bash
rtk git add src/lib/components/email/ComposeModal.svelte src/lib/i18n/zh-CN.ts src/lib/i18n/en-US.ts src/lib/__tests__/components/ComposeModal.test.ts
rtk git commit -m "接入收件人 chip 与密送输入"
```

---

### Task 5: Final verification and cleanup

**Files:**
- Modify as needed based on verification failures only.

**Interfaces:**
- Consumes:
  - All interfaces from Tasks 1-4.
- Produces:
  - Verified M1 implementation.

- [ ] **Step 1: Run backend tests**

Run:

```bash
rtk bun run test:rust
```

Expected: all Rust tests pass. Existing dead-code warnings in test helpers are acceptable if already present.

- [ ] **Step 2: Run frontend tests**

Run:

```bash
rtk bun run test:frontend
```

Expected: all frontend tests pass.

- [ ] **Step 3: Run type and Svelte checks**

Run:

```bash
rtk bun run check
```

Expected: `svelte-check found 0 errors and 0 warnings`.

- [ ] **Step 4: Inspect worktree**

Run:

```bash
rtk git status --short
```

Expected: only M1 implementation files and the two docs from brainstorming are modified or untracked.

- [ ] **Step 5: Commit final verification task if commits are allowed**

Only if the user has explicitly allowed commits for implementation tasks and earlier tasks were not already committed:

```bash
rtk git add docs/superpowers/specs/2026-07-08-email-compose-milestones-design.md docs/superpowers/specs/2026-07-08-email-compose-m1-recipients-design.md docs/superpowers/plans/2026-07-08-email-compose-m1-recipients.md src-tauri/src/service/email_address.rs src-tauri/src/service/mod.rs src-tauri/src/service/email_service.rs src-tauri/src/command/email.rs src-tauri/src/lib.rs src-tauri/tests/email_commands.rs src/lib/bindings.ts src/lib/components/email/recipient.ts src/lib/components/email/RecipientField.svelte src/lib/components/email/ComposeModal.svelte src/lib/i18n/zh-CN.ts src/lib/i18n/en-US.ts src/lib/__tests__/components/RecipientField.test.ts src/lib/__tests__/components/ComposeModal.test.ts
rtk git commit -m "实现发件收件人 chip 输入"
```

---

## Self-Review Checklist

- Spec coverage:
  - To/Cc/Bcc fields: Task 4.
  - chip/token display and deletion: Tasks 2-4.
  - complex address parsing via Rust `mail-parser`: Task 1.
  - no frontend parser dependency: Global Constraints and Task 1.
  - valid-address-only draft save: Task 4.
  - invalid chip blocks send: Task 4.
  - Bcc not in MIME header: Task 1 and final verification.
  - tests: Tasks 1-5.

- Placeholder scan:
  - No `TBD`, `TODO`, unchecked vague “write tests for this” steps.
  - Each task includes exact files, commands, and expected outcomes.

- Type consistency:
  - Backend response type is `ParseEmailAddressesResponse`.
  - Frontend command wrapper is `commands.parseEmailAddresses`.
  - Frontend chip type is `RecipientChip`.
  - Valid address extraction function is `validEmails`.
