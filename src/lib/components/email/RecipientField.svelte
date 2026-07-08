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

	let { label, chips, placeholder = "", testIdPrefix, onChange }: Props = $props();

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
				parseError = String(result.error.message ?? "解析收件人失败");
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
		} else if (event.key === " ") {
			const value = inputValue.trim();
			if (value.includes("@") && !/[<>]/.test(value)) {
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
