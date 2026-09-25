<script lang="ts">
  let { count = 8 }: { count?: number } = $props()
</script>

<div class="grid grid-cols-2 gap-x-3 gap-y-6 sm:gap-x-5 md:grid-cols-3 lg:grid-cols-4">
  {#each { length: count }, i (i)}
    <div>
      <div class="skeleton aspect-[4/3] w-full"></div>
      <div class="skeleton mt-3 h-4 w-4/5"></div>
      <div class="skeleton mt-2 h-3 w-2/5"></div>
    </div>
  {/each}
</div>
