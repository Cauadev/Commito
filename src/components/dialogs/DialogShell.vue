<script setup lang="ts">
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from '@/components/ui/dialog'
import { cn } from '@/lib/utils'

const props = withDefaults(
  defineProps<{ title: string; description?: string; width?: string; open?: boolean }>(),
  { width: 'sm:max-w-lg', open: true },
)
const emit = defineEmits<{ close: [] }>()
</script>

<template>
  <Dialog :open="props.open" @update:open="(v: boolean) => !v && emit('close')">
    <DialogContent :class="cn('gap-3 bg-popover p-5', props.width)" @open-auto-focus="(e: Event) => e.preventDefault()">
      <DialogHeader class="gap-1">
        <DialogTitle class="text-[15px] font-semibold">{{ props.title }}</DialogTitle>
        <DialogDescription v-if="props.description" class="text-[12.5px] leading-relaxed text-muted-foreground">
          {{ props.description }}
        </DialogDescription>
        <DialogDescription v-else class="sr-only">{{ props.title }}</DialogDescription>
      </DialogHeader>
      <div class="grid gap-3">
        <slot />
      </div>
      <DialogFooter v-if="$slots.footer" class="mt-1 gap-2">
        <slot name="footer" />
      </DialogFooter>
    </DialogContent>
  </Dialog>
</template>
