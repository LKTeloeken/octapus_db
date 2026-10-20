import { gsap } from 'gsap';
import { blink, breathe, look, sway, wave, type Scene } from '../motion';
import { thoughtDots } from '../props';
import { REST, armTo } from '../rig';

/**
 * Pensando: o polvo respira, balança a cabeça e ondula os tentáculos; a cada
 * ciclo ergue um tentáculo ao lado da cabeça, tamborila no ar, olha para
 * cima e solta as bolhas de pensamento.
 */
export const thinking: Scene = {
  id: 'thinking',
  label: 'Pensando',
  phase: 'base de todas',
  loop: true,
  build(rig) {
    breathe(rig);
    sway(rig);
    blink(rig);
    for (const [name, delay] of [
      ['a', 0],
      ['b', 0.35],
      ['c', 0.7],
    ] as const) {
      wave(rig.arms[name], {
        dx: name === 'a' ? -14 : name === 'b' ? -10 : 10,
        dy: 12,
        delay,
      });
    }

    const { g: bubbles, dots } = thoughtDots(rig.front);
    gsap.set(bubbles, { x: 700, y: 60 });
    gsap.set(dots, { scale: 0, transformOrigin: '50% 50%' });

    const d = rig.arms.d;
    // "Hmm…": o tentáculo sobe ao lado da cabeça, com a ponta para cima e
    // sem encostar nela (a silhueta continua a do logo)
    const raised = {
      p0: { ...REST.d.p0 },
      p1: { x: 700, y: 420 },
      p2: { x: 770, y: 300 },
      p3: { x: 740, y: 190 },
    };

    const tl = gsap.timeline({ repeat: -1, repeatDelay: 0.6 });
    tl.add(armTo(d, raised, { duration: 0.55, ease: 'power2.inOut' }), 0.4)
      // Antecipação: o corpo cede um pouco antes de "pensar"
      .to(
        rig.eyes.group,
        { x: 14, y: -16, duration: 0.35, ease: 'power2.out' },
        '<0.2',
      )
      // A ponta tamborila no ar
      .to(d.pose.p3, {
        x: '-=14',
        y: '-=10',
        duration: 0.12,
        ease: 'power2.out',
        yoyo: true,
        repeat: 3,
      })
      .to(
        dots,
        { scale: 1, duration: 0.28, ease: 'back.out(2)', stagger: 0.18 },
        '<',
      )
      .to(
        dots,
        { opacity: 0, duration: 0.3, ease: 'power1.in', stagger: 0.12 },
        '+=0.9',
      )
      .add(armTo(d, REST.d, { duration: 0.6, ease: 'power2.inOut' }), '<')
      .add(look(rig, 0, 0, { duration: 0.4 }), '<')
      .set(dots, { scale: 0, opacity: 0.5 });

    return { timeline: tl, stillAt: 0.45 };
  },
};
