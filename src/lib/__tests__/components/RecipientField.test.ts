import { fireEvent, render, screen, waitFor } from "@testing-library/svelte";
import { beforeEach, describe, expect, it, vi } from "vitest";
import RecipientField from "$lib/components/email/RecipientField.svelte";
import {
	chipsFromParsedResponse,
	hasInvalidRecipients,
	recipientLabel,
	validEmails,
} from "$lib/components/email/recipient";
import { mockInvoke } from "../mocks/tauri";

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
			{
				id: "1",
				name: "Alice",
				raw: "Alice <alice@example.com>",
				email: "alice@example.com",
				valid: true,
			},
			{ id: "2", raw: "bad", valid: false, reason: "bad" },
		];

		expect(validEmails(chips)).toEqual(["alice@example.com"]);
		expect(hasInvalidRecipients(chips)).toBe(true);
		expect(chips[0]).toBeDefined();
		expect(chips[1]).toBeDefined();
		expect(recipientLabel(chips[0]!)).toBe("Alice <alice@example.com>");
		expect(recipientLabel(chips[1]!)).toBe("bad");
	});
});

describe("RecipientField component", () => {
	beforeEach(() => {
		mockInvoke.mockReset();
	});

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

	it("parses bare email on Space", async () => {
		const onChange = vi.fn();
		mockInvoke.mockResolvedValue({
			addresses: [{ name: null, email: "alice@example.com", raw: "alice@example.com" }],
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
		await fireEvent.input(input, { target: { value: "alice@example.com" } });
		await fireEvent.keyDown(input, { key: " " });

		await waitFor(() => {
			expect(onChange).toHaveBeenCalledWith([
				expect.objectContaining({
					email: "alice@example.com",
					valid: true,
				}),
			]);
		});
		expect(mockInvoke).toHaveBeenCalledWith("parse_email_addresses", {
			input: "alice@example.com",
		});
	});

	it("does not parse display-name address on Space", async () => {
		const onChange = vi.fn();
		mockInvoke.mockResolvedValue({
			addresses: [],
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
		await fireEvent.keyDown(input, { key: " " });

		expect(onChange).not.toHaveBeenCalled();
		expect(mockInvoke).not.toHaveBeenCalledWith("parse_email_addresses", {
			input: "Alice <alice@example.com>",
		});
	});

	it("renders invalid chips and lets the user delete chips", async () => {
		const onChange = vi.fn();
		render(RecipientField, {
			props: {
				label: "收件人",
				testIdPrefix: "to",
				chips: [{ id: "1", raw: "bad", valid: false, reason: "无法解析邮件地址" }],
				onChange,
			},
		});

		expect(screen.getByText("bad")).toBeTruthy();
		await fireEvent.click(screen.getByTestId("to-recipient-remove-1"));

		expect(onChange).toHaveBeenCalledWith([]);
	});
});
