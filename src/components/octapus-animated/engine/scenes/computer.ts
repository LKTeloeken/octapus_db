import { gsap } from 'gsap';
import type { ArmPose } from '../geometry';
import { blink, breathe, wave, type Scene } from '../motion';
import { laptopBack } from '../props';
import { REST, type ArmName } from '../rig';

/** Onde fica o notebook: ao lado do polvo, à direita, no chão */
const AT = { x: 770, y: 742 };

/**
 * Computador (escrevendo a consulta): o notebook ao lado, de costas para
 * nós e com a tela virada para o polvo. Dois tentáculos passam por trás da
 * tampa até o teclado e digitam alternando; o polvo olha para a tela e as
 * linhas de código sobem acima da tampa, uma a uma.
 */
export const computerScene: Scene = {
  id: 'computer',
  label: 'Computador',
  phase: 'generating · escrevendo',
  loop: true,
  build(rig) {
    breathe(rig, 6, 0.9);
    blink(rig);
    wave(rig.arms.a, { dx: -12, dy: 14 });
    wave(rig.arms.b, { dx: -8, dy: -8, delay: 0.4 });

    const pc = laptopBack(rig.front);
    const pcCut = laptopBack(rig.cutFront, true);
    gsap.set([pc.g, pcCut.g], { x: AT.x, y: AT.y });
    // Olha para a tela, com a cabeça um pouco inclinada para ela
    gsap.set(rig.eyes.group, { x: 24, y: 14 });
    gsap.set(rig.figure, { rotation: 2, svgOrigin: '420 560' });

    // Os dois tentáculos da direita entram por trás da tampa (as pontas
    // somem atrás dela, no teclado)
    const keys: Record<'c' | 'd', ArmPose> = {
      c: {
        p0: { ...REST.c.p0 },
        p1: { x: 490, y: 600 },
        p2: { x: 550, y: 725 },
        p3: { x: 690, y: 700 },
      },
      d: {
        p0: { ...REST.d.p0 },
        p1: { x: 690, y: 370 },
        p2: { x: 790, y: 410 },
        p3: { x: 800, y: 600 },
      },
    };
    for (const name of ['c', 'd'] as const) {
      const pose = rig.arms[name].pose;
      for (const key of ['p1', 'p2', 'p3'] as const)
        gsap.set(pose[key], keys[name][key]);
    }

    // Digitação: as pontas batem alternando, rápido e sem ritmo perfeito; o
    // trecho visível do tentáculo acompanha
    const typing = gsap.timeline({ repeat: -1 });
    const taps: [ArmName, number][] = [
      ['d', 0],
      ['c', 0.14],
      ['d', 0.26],
      ['d', 0.36],
      ['c', 0.52],
      ['c', 0.62],
      ['d', 0.78],
    ];
    for (const [name, at] of taps) {
      const pose = rig.arms[name].pose;
      typing
        .to(
          pose.p3,
          {
            y: '-=22',
            duration: 0.06,
            ease: 'power2.out',
            yoyo: true,
            repeat: 1,
          },
          at,
        )
        .to(
          pose.p2,
          {
            y: '-=12',
            duration: 0.06,
            ease: 'power2.out',
            yoyo: true,
            repeat: 1,
          },
          at,
        );
    }
    typing.to({}, { duration: 0.12 });

    // As linhas aparecem acima da tampa (crescendo da esquerda, como texto),
    // sobem e somem — uma depois da outra
    const tl = gsap.timeline({ repeat: -1 });
    gsap.set(pc.lines, {
      y: -250,
      scaleX: 0,
      opacity: 0.7,
      transformOrigin: '0% 50%',
    });
    pc.lines.forEach((line, i) => {
      const at = i * 0.55;
      tl.to(line, { scaleX: 1, duration: 0.45, ease: 'steps(6)' }, at)
        .to(
          line,
          { y: -380, opacity: 0, duration: 0.9, ease: 'power1.in' },
          at + 0.55,
        )
        // Os olhos seguem a linha nova
        .to(
          rig.eyes.group,
          { x: 24 + i * 2, y: 12 - i * 3, duration: 0.25, ease: 'power2.out' },
          at,
        );
    });
    tl.set(pc.lines, { y: -250, scaleX: 0, opacity: 0.7 }, '+=0.1');

    // O notebook entra de baixo na primeira vez
    gsap.from([pc.g, pcCut.g], {
      y: AT.y + 120,
      opacity: 0,
      duration: 0.4,
      ease: 'back.out(1.4)',
    });
    return { timeline: tl, stillAt: 0.35 };
  },
};
