<script setup lang="ts">
defineProps<{ label?: string; hint?: string; error?: string | null }>()
</script>

<template>
  <label class="grid gap-1.5">
    <span v-if="label" class="text-[12px] font-medium text-muted-foreground">{{ label }}</span>
    <slot />
    <span v-if="error" class="text-[11.5px] text-destructive">{{ error }}</span>
    <span v-else-if="hint" class="text-[11.5px] leading-snug text-faint">{{ hint }}</span>
  </label>
</template>
