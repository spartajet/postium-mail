<script lang="ts">
  import EmailList from '$lib/components/email/EmailList.svelte';
  import EmailDetail from '$lib/components/email/EmailDetail.svelte';
</script>

<div class="flex h-full w-full">
  <EmailList />
  <div class="flex-1">
    <EmailDetail />
  </div>
</div>
