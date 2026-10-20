import { gsap } from 'gsap';
import { breathe, eyesScaleY, waveFree, type Scene } from '../motion';
import { clipboard } from '../props';

/** Centro da prancheta: ao lado do polvo, à direita, na altura do rosto */
const AT = { x: 800, y: 450 };
const SCALE = 0.8;

/**
 * Prancheta (conferindo a consulta: guard e preflight): o tentáculo da
 * direita segura a prancheta ao lado, pela borda; os olhos descem linha a
 * linha e cada visto se desenha; no último, um aceno de cabeça. Recomeça com
 * os vistos apagados.
 */
export const clipboardScene: Scene = {
  id: 'clipboard',
  label: 'Prancheta',
  phase: 'checking · conferindo',
  loop: true,
  build(rig) {
    breathe(rig, 8);
    waveFree(rig, ['d']);

    const board = clipboard(rig.front);
    const boardCut = clipboard(rig.cutFront, true);
    const both = [board.g, boardCut.g];
    gsap.set(both, {
      x: AT.x,
      y: AT.y,
      rotation: 4,
      transformOrigin: '50% 50%',
      scale: SCALE,
    });

    // O tentáculo vem da raiz para a direita e segura a borda esquerda
    const d = rig.arms.d;
    d.grip = true;
    const edge = AT.x - 150 * SCALE + 10;
    gsap.set(d.pose.p1, { x: 625, y: 425 });
    gsap.set(d.pose.p2, { x: edge - 50, y: AT.y + 70 });
    gsap.set(d.pose.p3, { x: edge, y: AT.y + 40 });

    // A prancheta balança de leve com a respiração (e a ponta junto)
    gsap.to(both, {
      rotation: -1,
      duration: 1.4,
      ease: 'sine.inOut',
      yoyo: true,
      repeat: -1,
    });
    gsap.to(d.pose.p3, {
      y: '-=8',
      duration: 1.4,
      ease: 'sine.inOut',
      yoyo: true,
      repeat: -1,
    });

    const { group } = rig.eyes;
    const tl = gsap.timeline({ repeat: -1, repeatDelay: 0.5 });
    tl.to(group, { x: 26, y: -4, duration: 0.3, ease: 'power2.out' }, 0);
    board.checks.forEach((check, i) => {
      const at = 0.3 + i * 0.6;
      tl.to(
        group,
        { x: 26, y: -4 + i * 12, duration: 0.2, ease: 'power2.out' },
        at,
      )
        .to(
          board.bars[i],
          { opacity: 1, duration: 0.15, ease: 'power1.out' },
          at,
        )
        .to(
          check,
          { strokeDashoffset: 0, duration: 0.25, ease: 'power2.out' },
          at + 0.2,
        );
    });
    // Aceno: a cabeça desce e sobe, olhos semicerrados de satisfação
    tl.to(
      rig.figure,
      {
        rotation: 4,
        svgOrigin: '420 560',
        duration: 0.15,
        ease: 'power2.out',
        yoyo: true,
        repeat: 1,
      },
      '+=0.15',
    )
      .add(eyesScaleY(rig, 0.55, { duration: 0.15 }), '<')
      .add(
        eyesScaleY(rig, 1, { duration: 0.2, ease: 'power2.inOut' }),
        '+=0.35',
      )
      .to(group, { x: 12, y: 0, duration: 0.3, ease: 'power2.inOut' }, '<')
      .to(
        board.checks,
        { strokeDashoffset: 1, duration: 0.2, ease: 'power1.in' },
        '+=0.3',
      )
      .to(board.bars, { opacity: 0.55, duration: 0.2 }, '<');

    return { timeline: tl, stillAt: 0.7 };
  },
};
