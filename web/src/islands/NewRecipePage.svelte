<script lang="ts">
  import RecipeEditor from "../components/RecipeEditor.svelte"
  import { api, errorMessage } from "../lib/api"
  import type { Recipe, RecipeFields } from "../lib/recipe"
  import { toast } from "../lib/toast"

  let saving = $state(false)

  async function save(fields: RecipeFields) {
    saving = true
    try {
      const recipe = await api<Recipe>("/api/recipes", { method: "POST", body: fields })
      location.replace(`/recipes/${recipe.id}`)
    } catch (e) {
      toast({ title: "Couldn't save", description: errorMessage(e), tone: "error" })
      saving = false
    }
  }
</script>

<RecipeEditor {saving} submitLabel="Save recipe" onsave={save} oncancel={() => history.back()} />
