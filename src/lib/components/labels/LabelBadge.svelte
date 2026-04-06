<script lang="ts">
  import { X } from 'lucide-svelte';

  let {
    label,
    removable = false,
    onremove,
  }: {
    label: { name: string; color: string };
    removable?: boolean;
    onremove?: () => void;
  } = $props();
</script>

<span
  class="inline-flex items-center gap-1 rounded-full px-2 py-0.5 text-xs font-medium"
  style="background-color: {label.color}20; color: {label.color}"
>
  <span class="h-1.5 w-1.5 rounded-full" style="background-color: {label.color}"></span>
  {label.name}
  {#if removable}
    <button class="ml-0.5 hover:opacity-70" onclick={onremove}>
      <X size={12} style="color: {label.color}" />
    </button>
  {/if}
</span>
