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

export function chipsFromParsedResponse(response: ParseEmailAddressesResponse): RecipientChip[] {
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

export function mergeRecipientChips(existing: RecipientChip[], incoming: RecipientChip[]) {
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
