<script setup lang="ts">
import { cva, type VariantProps } from 'class-variance-authority'
import { cn } from '@/lib/utils'

const buttonVariants = cva(
  'inline-flex items-center justify-center gap-1.5 whitespace-nowrap rounded-lg text-xs font-medium transition-all focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary/35 active:scale-[0.985] disabled:pointer-events-none disabled:opacity-50',
  {
    variants: {
      variant: {
        default: 'bg-primary text-white shadow-xs hover:brightness-95',
        secondary: 'border border-border bg-card text-foreground shadow-2xs hover:border-primary/45 hover:bg-primary-soft hover:text-primary',
        outline: 'border border-border bg-transparent text-foreground hover:bg-accent',
        ghost: 'bg-transparent text-muted-foreground hover:bg-accent hover:text-foreground',
        destructive: 'border border-destructive/40 bg-destructive/10 text-destructive hover:bg-destructive/15',
      },
      size: {
        default: 'h-8 px-3.5',
        sm: 'h-7 px-2.5 text-[11.5px] rounded-md',
        icon: 'h-7 w-7 p-0 rounded-md',
      },
    },
    defaultVariants: {
      variant: 'secondary',
      size: 'default',
    },
  },
)

type ButtonVariants = VariantProps<typeof buttonVariants>

const props = withDefaults(defineProps<{
  variant?: ButtonVariants['variant']
  size?: ButtonVariants['size']
  type?: 'button' | 'submit' | 'reset'
  class?: string
}>(), {
  type: 'button',
})
</script>

<template>
  <button :type="props.type" :class="cn(buttonVariants({ variant: props.variant, size: props.size }), props.class)">
    <slot />
  </button>
</template>
