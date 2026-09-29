/**
 * Specs de movimento das sobreposições (DESIGN.md §6), em classes do
 * tw-animate-css. Entram com `ease-out` e saem mais rápido com `ease-in`; só
 * opacidade, escala e deslocamento — nada que dispare layout. Com
 * `prefers-reduced-motion` o globals.css zera escala e deslocamento.
 */

/** Menu, popover e select: nascem do gatilho (origem no `transform-origin` do Radix). */
export const POPPER_MOTION = [
  'data-[state=open]:animate-in data-[state=closed]:animate-out',
  'data-[state=open]:fade-in-0 data-[state=closed]:fade-out-0',
  'data-[state=open]:zoom-in-96 data-[state=closed]:zoom-out-96',
  'data-[side=bottom]:slide-in-from-top-1 data-[side=top]:slide-in-from-bottom-1',
  'data-[side=left]:slide-in-from-right-1 data-[side=right]:slide-in-from-left-1',
  'duration-180 ease-out data-[state=closed]:duration-120 data-[state=closed]:ease-in',
].join(' ');

/** Tooltip: fade + 2 px, sempre curto. */
export const TOOLTIP_MOTION = [
  'animate-in fade-in-0 data-[state=closed]:animate-out data-[state=closed]:fade-out-0',
  'data-[side=bottom]:slide-in-from-top-[2px] data-[side=top]:slide-in-from-bottom-[2px]',
  'data-[side=left]:slide-in-from-right-[2px] data-[side=right]:slide-in-from-left-[2px]',
  'duration-120 ease-out data-[state=closed]:ease-in',
].join(' ');

/** Véu sob diálogos. */
export const SCRIM_MOTION = [
  'data-[state=open]:animate-in data-[state=closed]:animate-out',
  'data-[state=open]:fade-in-0 data-[state=closed]:fade-out-0',
  'duration-180 data-[state=closed]:duration-120',
].join(' ');

/** Diálogo e paleta: sobem 8 px e crescem de 98%. */
export const DIALOG_MOTION = [
  'data-[state=open]:animate-in data-[state=closed]:animate-out',
  'data-[state=open]:fade-in-0 data-[state=closed]:fade-out-0',
  'data-[state=open]:zoom-in-98 data-[state=closed]:zoom-out-98',
  'data-[state=open]:slide-in-from-bottom-2',
  'duration-240 ease-out data-[state=closed]:duration-120 data-[state=closed]:ease-in',
].join(' ');
