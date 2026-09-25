<script lang="ts">
  /** The quiet note on a recipe page when Wee Chef tidied or flagged lines on import. */
  import ChefHat from "@lucide/svelte/icons/chef-hat"
  import { api, errorMessage } from "../lib/api"
  import { fixText, plural } from "../lib/checks"
  import type { Recipe, RecipeChecks } from "../lib/recipe"
  import { toast } from "../lib/toast"

  interface Props {
    recipeId: number
    checks: RecipeChecks
    /** After Undo: the recipe as imported, and the check without its fixes. */
    onundo: (recipe: Recipe, checks: RecipeChecks) => void
    class?: string
  }
  let { recipeId, checks, onundo, class: klass = "" }: Props = $props()

  const fixed = $derived(checks.flags.filter((f) => f.state === "fixed"))
  const review = $derived(checks.flags.filter((f) => f.state === "review"))
  let open = $state(false)
  let undoing = $state(false)

  async function undo() {
    if (undoing) return
    undoing = true
    try {
      const res = await api<{ recipe: Recipe; checks: RecipeChecks }>(
        `/api/recipes/${recipeId}/checks/undo`,
        { method: "POST" },
      )
      onundo(res.recipe, res.checks)
      toast({ title: "Put back as it was imported" })
    } catch (e) {
      toast({ title: "Couldn't undo", description: errorMessage(e), tone: "error" })
    } finally {
      undoing = false
    }
  }
</script>

{#if fixed.length || review.length}
  <aside class={["card no-print px-4 py-1", klass]} aria-label="Wee Chef">
    {#if fixed.length}
      <div class="flex items-center gap-x-2">
        <ChefHat class="text-primary size-[18px] flex-none" aria-hidden="true" />
        <span class="min-w-0 flex-1 py-2.5 text-[15px] leading-snug font-bold">
          Wee Chef tidied {plural(fixed.length, "thing")}
        </span>
        <span class="flex flex-none items-center gap-x-2">
          <button
            type="button"
            class="link inline-flex min-h-11 items-center"
            aria-expanded={open}
            onclick={() => (open = !open)}
          >
            {open ? "Hide" : "See"}
          </button>
          {#if checks.canUndo}
            <span class="text-ink-muted" aria-hidden="true">·</span>
            <button
              type="button"
              class="link inline-flex min-h-11 items-center"
              disabled={undoing}
              onclick={undo}
            >
              Undo
            </button>
          {/if}
        </span>
      </div>
      {#if open}
        <ul class="text-ink-muted space-y-1.5 pb-3 pl-[26px] text-sm">
          {#each fixed as f (f.id)}
            <li class="break-words">{fixText(f)}</li>
          {/each}
        </ul>
      {/if}
    {/if}
    {#if review.length}
      <div class={["flex items-center gap-x-2", fixed.length && "border-line border-t"]}>
        {#if fixed.length}
          <span class="w-[18px] flex-none" aria-hidden="true"></span>
        {:else}
          <ChefHat class="text-primary size-[18px] flex-none" aria-hidden="true" />
        {/if}
        <span class="min-w-0 flex-1 py-2.5 text-[15px] leading-snug">
          {plural(review.length, "line")} might need a look
        </span>
        <a
          href={`/recipes/${recipeId}/edit`}
          class="link inline-flex min-h-11 flex-none items-center"
        >
          Edit
        </a>
      </div>
    {/if}
  </aside>
{/if}
