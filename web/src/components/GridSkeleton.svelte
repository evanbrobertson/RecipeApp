<script lang="ts">
  let { count = 8 }: { count?: number } = $props()
</script>

<div class="grid grid-cols-2 gap-x-3 gap-y-5 sm:grid-cols-3 sm:gap-x-4 lg:grid-cols-4 xl:grid-cols-5">
  {#each { length: count }, i (i)}
    <div>
      <div class="skeleton aspect-[4/3] w-full"></div>
      <div class="skeleton mt-2.5 h-4 w-4/5"></div>
      <div class="skeleton mt-2 h-3.5 w-2/5"></div>
    </div>
  {/each}
</div>
