/**
 * O motor do polvo animado (rig + cenas + GSAP). Só entra por `import()` —
 * o GSAP vem num chunk próprio, carregado quando o primeiro polvo aparece.
 * Nasceu no laboratório `octapus-mascot-lab` (fora do repo): cena nova se
 * desenha lá e é copiada para cá.
 */
export { Stage } from './director';
export { SCENES } from './scenes';
