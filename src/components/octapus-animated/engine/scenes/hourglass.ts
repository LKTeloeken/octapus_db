import { gsap } from 'gsap';
import { blink, breathe, waveFree, type Scene } from '../motion';
import { hourglass } from '../props';

/** A ampulheta fica no chão, à direita */
const AT = { x: 790, y: 640 };

/**
 * Ampulheta (esperando a vez — rate limit): a areia escorre, a ampulheta vira;
 * o tentáculo da direita tamborila no chão, impaciente, e o polvo suspira
 * (o corpo cede e volta) olhando para ela.
 */
export const hourglassScene: Scene = {
  id: 'hourglass',
  label: 'Ampulheta',
  phase: 'waiting · aguardando a vez',
  loop: true,
  build(rig) {
    breathe(rig, 6, 1.8);
    blink(rig);
    waveFree(rig, ['d']);

    const glass = hourglass(rig.front);
    const glassCut = hourglass(rig.cutFront, true);
    const both = [glass.g, glassCut.g];
    gsap.set(both, {
      x: AT.x,
      y: AT.y,
      scale: 0.95,
      transformOrigin: '50% 50%',
    });
    gsap.set(glass.top, { transformOrigin: '50% 100%' });
    gsap.set(glass.bottom, { scaleY: 0, transformOrigin: '50% 100%' });
    gsap.set(rig.eyes.group, { x: 22, y: 12 });

    // O tentáculo da direita apoia a ponta na borda de cima da ampulheta e
    // tamborila
    const d = rig.arms.d.pose;
    gsap.set(d.p1, { x: 630, y: 420 });
    gsap.set(d.p2, { x: 690, y: 420 });
    gsap.set(d.p3, { x: AT.x - 50, y: AT.y - 200 });
    const tapping = gsap.timeline({ repeat: -1, repeatDelay: 0.5 });
    tapping
      .to(d.p3, {
        y: '-=34',
        duration: 0.09,
        ease: 'power2.out',
        yoyo: true,
        repeat: 1,
      })
      .to(
        d.p3,
        {
          y: '-=34',
          duration: 0.09,
          ease: 'power2.out',
          yoyo: true,
          repeat: 1,
        },
        '+=0.1',
      )
      .to(
        d.p3,
        {
          y: '-=34',
          duration: 0.09,
          ease: 'power2.out',
          yoyo: true,
          repeat: 1,
        },
        '+=0.1',
      );

    const tl = gsap.timeline({ repeat: -1 });
    tl.to(glass.top, { scaleY: 0, duration: 1.6, ease: 'none' }, 0)
      .to(glass.bottom, { scaleY: 1, duration: 1.6, ease: 'none' }, 0)
      .to(glass.stream, { opacity: 0, duration: 0.1 }, 1.5)
      // Suspiro: o corpo cede e volta
      .to(
        rig.figure,
        {
          scaleY: 0.96,
          svgOrigin: '420 780',
          duration: 0.35,
          ease: 'sine.inOut',
          yoyo: true,
          repeat: 1,
        },
        1.2,
      )
      // Vira (antecipação: recua um pouco antes de girar)
      .to(both, { rotation: -8, duration: 0.12, ease: 'power2.out' }, 1.7)
      .to(both, { rotation: 180, duration: 0.45, ease: 'back.out(1.6)' })
      .set(both, { rotation: 0 })
      .set(glass.top, { scaleY: 1 })
      .set(glass.bottom, { scaleY: 0 })
      .set(glass.stream, { opacity: 0.6 });

    return { timeline: tl, stillAt: 0.4 };
  },
};
