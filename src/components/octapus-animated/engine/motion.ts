import { gsap } from 'gsap';
import { clonePose, type ArmPose } from './geometry';
import {
  ARM_NAMES,
  EYE_ORIGINS,
  REST,
  armTo,
  type Arm,
  type ArmName,
  type Octapus,
} from './rig';

/**
 * Peças de movimento usadas por todas as cenas. Regras tiradas do artigo da
 * Codrops e dos 12 princípios: antecipação antes do gesto, easings diferentes
 * para subir e descer, uma pequena passada antes de assentar, e as partes de
 * um mesmo gesto disparando juntas (`"<"`).
 */

export interface SceneHandle {
  /** A timeline principal (em loop, ou uma vez) */
  timeline: gsap.core.Timeline;
  /** Pose parada para movimento reduzido (progresso da timeline, 0–1) */
  stillAt: number;
}

export interface Scene {
  id: string;
  label: string;
  /** Fase do turno que a cena representa */
  phase: string;
  /** `false` = toca uma vez e segura o fim */
  loop: boolean;
  build: (rig: Octapus) => SceneHandle;
}

/** Respirar: sobe e desce o corpo todo, devagar (a partir de onde está) */
export function breathe(rig: Octapus, amount = 16, duration = 1.4) {
  return gsap.to(rig.figure, {
    y: `-=${amount}`,
    duration,
    ease: 'sine.inOut',
    yoyo: true,
    repeat: -1,
  });
}

/** Inclinar a cabeça de um lado para o outro */
export function sway(rig: Octapus, degrees = 3, duration = 2.8) {
  gsap.set(rig.figure, { rotation: -degrees, svgOrigin: '420 560' });
  return gsap.to(rig.figure, {
    rotation: degrees,
    svgOrigin: '420 560',
    duration,
    ease: 'sine.inOut',
    yoyo: true,
    repeat: -1,
  });
}

/** Piscar em intervalos irregulares (às vezes duas vezes seguidas) */
export function blink(rig: Octapus) {
  const { right, left } = rig.eyes;
  const tl = gsap.timeline({
    repeat: -1,
    repeatDelay: 2.2,
    repeatRefresh: true,
    delay: 0.8,
  });
  const once = (at: number | string) => {
    tl.to(
      right,
      {
        scaleY: 0.08,
        svgOrigin: EYE_ORIGINS.right,
        duration: 0.07,
        ease: 'power2.in',
      },
      at,
    )
      .to(
        left,
        {
          scaleY: 0.08,
          svgOrigin: EYE_ORIGINS.left,
          duration: 0.07,
          ease: 'power2.in',
        },
        '<',
      )
      .to(right, {
        scaleY: 1,
        svgOrigin: EYE_ORIGINS.right,
        duration: 0.12,
        ease: 'power2.out',
      })
      .to(
        left,
        {
          scaleY: 1,
          svgOrigin: EYE_ORIGINS.left,
          duration: 0.12,
          ease: 'power2.out',
        },
        '<',
      );
  };
  once(0);
  // De vez em quando, uma segunda piscada logo depois
  tl.call(() => {
    if (Math.random() < 0.3) {
      const extra = gsap.timeline();
      extra
        .to(
          [right, left],
          { scaleY: 0.08, duration: 0.06, ease: 'power2.in' },
          0.12,
        )
        .to([right, left], { scaleY: 1, duration: 0.1, ease: 'power2.out' });
    }
  });
  return tl;
}

/** Olhar para um lado (desloca os dois olhos) */
export function look(
  rig: Octapus,
  x: number,
  y: number,
  vars: gsap.TweenVars = {},
) {
  return gsap.to(rig.eyes.group, {
    x,
    y,
    duration: 0.3,
    ease: 'power2.out',
    ...vars,
  });
}

/** Ondular um tentáculo livre: a ponta e o meio vão e voltam */
export function wave(
  arm: Arm,
  { dx = 18, dy = 14, duration = 1.4, delay = 0 } = {},
) {
  const tl = gsap.timeline({ repeat: -1, yoyo: true, delay });
  tl.to(
    arm.pose.p3,
    { x: `+=${dx}`, y: `-=${dy}`, duration, ease: 'sine.inOut' },
    0,
  ).to(
    arm.pose.p2,
    { x: `+=${dx * 0.5}`, y: `-=${dy * 0.6}`, duration, ease: 'sine.inOut' },
    0,
  );
  return tl;
}

/** Os tentáculos que a cena não usa ondulam sozinhos */
export function waveFree(rig: Octapus, busy: ArmName[] = []) {
  const settings: Record<ArmName, { dx: number; dy: number; delay: number }> = {
    a: { dx: -14, dy: 16, delay: 0 },
    b: { dx: -10, dy: -10, delay: 0.35 },
    c: { dx: 10, dy: -10, delay: 0.7 },
    d: { dx: 14, dy: 16, delay: 1.05 },
  };
  return ARM_NAMES.filter(name => !busy.includes(name)).map(name =>
    wave(rig.arms[name], settings[name]),
  );
}

/** Pose relativa ao repouso: soma deslocamentos ponto a ponto */
export function poseFrom(
  name: ArmName,
  offsets: Partial<Record<keyof ArmPose, { x?: number; y?: number }>>,
): ArmPose {
  const pose = clonePose(REST[name]);
  for (const key of Object.keys(offsets) as (keyof ArmPose)[]) {
    pose[key].x += offsets[key]?.x ?? 0;
    pose[key].y += offsets[key]?.y ?? 0;
  }
  return pose;
}

/** Leva todos os tentáculos ao repouso */
export function toRest(rig: Octapus, duration = 0.35) {
  const tl = gsap.timeline();
  for (const name of ARM_NAMES) {
    tl.add(
      armTo(rig.arms[name], REST[name], { duration, ease: 'power2.inOut' }),
      0,
    );
  }
  tl.to(rig.eyes.group, { x: 0, y: 0, duration, ease: 'power2.inOut' }, 0).to(
    rig.figure,
    {
      x: 0,
      y: 0,
      rotation: 0,
      scaleX: 1,
      scaleY: 1,
      duration,
      ease: 'power2.inOut',
    },
    0,
  );
  return tl;
}

/** Passa um tentáculo para a frente da cabeça (com a fresta em volta) */
export function bringToFront(rig: Octapus, name: ArmName) {
  rig.arms[name].front = true;
}

export function sendBack(rig: Octapus, name: ArmName) {
  rig.arms[name].front = false;
}

/**
 * Abre/fecha os olhos (`scaleY` 1 = aberto) com o pivô no centro de cada um —
 * sem pivô, o olho escalaria a partir do canto do desenho.
 */
export function eyesScaleY(
  rig: Octapus,
  scaleY: number,
  vars: gsap.TweenVars = {},
) {
  const tl = gsap.timeline();
  tl.to(
    rig.eyes.right,
    {
      scaleY,
      svgOrigin: EYE_ORIGINS.right,
      duration: 0.12,
      ease: 'power2.out',
      ...vars,
    },
    0,
  ).to(
    rig.eyes.left,
    {
      scaleY,
      svgOrigin: EYE_ORIGINS.left,
      duration: 0.12,
      ease: 'power2.out',
      ...vars,
    },
    0,
  );
  return tl;
}

/**
 * Pivô no ponto de pega (0,0) do objeto: o `transformOrigin` do GSAP conta a
 * partir do canto da caixa do elemento, não da origem dele.
 */
export function gripOrigin(elements: SVGGraphicsElement[]) {
  for (const el of elements) {
    const box = el.getBBox();
    gsap.set(el, { transformOrigin: `${-box.x}px ${-box.y}px` });
  }
}
