import { clsx, type ClassValue } from 'clsx';
import { extendTailwindMerge } from 'tailwind-merge';

/**
 * O tailwind-merge precisa conhecer a escala do Ink (globals.css): sem isso ele
 * lê `text-body` como cor e o descarta quando aparece junto de `text-fg`.
 */
const twMerge = extendTailwindMerge({
  extend: {
    theme: {
      text: ['micro', 'small', 'body', 'heading', 'title', 'display'],
      shadow: ['raised', 'overlay', 'modal', 'control', 'cta', 'glass'],
      ease: ['standard', 'spring'],
      animate: ['indeterminate', 'shimmer'],
    },
  },
});

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}

export const tw = (strings: TemplateStringsArray) => strings[0];
