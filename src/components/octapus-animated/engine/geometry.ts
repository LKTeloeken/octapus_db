/** Geometria do rig: curvas cúbicas e o contorno de um tentáculo que afina. */

export interface Point {
  x: number;
  y: number;
}

/** Pose de um tentáculo: curva cúbica da base (p0) à ponta (p3) */
export interface ArmPose {
  p0: Point;
  p1: Point;
  p2: Point;
  p3: Point;
}

export const lerp = (a: number, b: number, t: number) => a + (b - a) * t;

export function cubic(pose: ArmPose, t: number): Point {
  const u = 1 - t;
  const a = u * u * u;
  const b = 3 * u * u * t;
  const c = 3 * u * t * t;
  const d = t * t * t;
  return {
    x: a * pose.p0.x + b * pose.p1.x + c * pose.p2.x + d * pose.p3.x,
    y: a * pose.p0.y + b * pose.p1.y + c * pose.p2.y + d * pose.p3.y,
  };
}

export function tangent(pose: ArmPose, t: number): Point {
  const u = 1 - t;
  const x =
    3 * u * u * (pose.p1.x - pose.p0.x) +
    6 * u * t * (pose.p2.x - pose.p1.x) +
    3 * t * t * (pose.p3.x - pose.p2.x);
  const y =
    3 * u * u * (pose.p1.y - pose.p0.y) +
    6 * u * t * (pose.p2.y - pose.p1.y) +
    3 * t * t * (pose.p3.y - pose.p2.y);
  const length = Math.hypot(x, y) || 1;
  return { x: x / length, y: y / length };
}

/** Ângulo (graus) da tangente na ponta: para alinhar o objeto segurado */
export const tipAngle = (pose: ArmPose) => {
  const t = tangent(pose, 1);
  return (Math.atan2(t.y, t.x) * 180) / Math.PI;
};

/**
 * Largura ao longo do tentáculo (0 = base, 1 = ponta): grossa na raiz, afina
 * no meio e volta a engrossar um pouco na ponta — o "bulbo" do logo.
 */
export interface WidthProfile {
  root: number;
  middle: number;
  tip: number;
}

function widthAt({ root, middle, tip }: WidthProfile, t: number) {
  // Interpolação suave em duas metades
  if (t < 0.55) {
    const k = t / 0.55;
    return lerp(root, middle, k * k * (3 - 2 * k));
  }
  const k = (t - 0.55) / 0.45;
  return lerp(middle, tip, k * k * (3 - 2 * k));
}

const fmt = (n: number) => Math.round(n * 10) / 10;

/**
 * Contorno fechado do tentáculo: a curva central amostrada, deslocada para os
 * dois lados pela largura, com ponta redonda (arco). Liso o bastante a 24
 * amostras com curvas quadráticas entre os pontos. `from` > 0 desenha só o
 * trecho final (do ponto `from` da curva até a ponta), também com a base
 * redonda — para os recortes de um tentáculo na frente do corpo.
 */
export function armOutline(
  pose: ArmPose,
  profile: WidthProfile,
  samples = 24,
  from = 0,
): string {
  const left: Point[] = [];
  const right: Point[] = [];

  for (let i = 0; i <= samples; i += 1) {
    const t = from + ((1 - from) * i) / samples;
    const p = cubic(pose, t);
    const d = tangent(pose, t);
    const half = widthAt(profile, t) / 2;
    left.push({ x: p.x - d.y * half, y: p.y + d.x * half });
    right.push({ x: p.x + d.y * half, y: p.y - d.x * half });
  }

  const smooth = (points: Point[]) => {
    let out = '';
    for (let i = 1; i < points.length - 1; i += 1) {
      const mid = {
        x: (points[i].x + points[i + 1].x) / 2,
        y: (points[i].y + points[i + 1].y) / 2,
      };
      out += ` Q ${fmt(points[i].x)} ${fmt(points[i].y)} ${fmt(mid.x)} ${fmt(mid.y)}`;
    }
    const last = points[points.length - 1];
    return `${out} L ${fmt(last.x)} ${fmt(last.y)}`;
  };

  const tipRadius = profile.tip / 2;
  const back = [...right].reverse();
  const start = left[0];
  return [
    `M ${fmt(start.x)} ${fmt(start.y)}`,
    smooth(left),
    // Ponta: meio círculo de um lado ao outro
    `A ${fmt(tipRadius)} ${fmt(tipRadius)} 0 0 0 ${fmt(back[0].x)} ${fmt(back[0].y)}`,
    smooth(back),
    // Base: também redonda (fica sob a cabeça)
    `A ${fmt(widthAt(profile, from) / 2)} ${fmt(widthAt(profile, from) / 2)} 0 0 0 ${fmt(start.x)} ${fmt(start.y)}`,
    'Z',
  ].join(' ');
}

/** Cópia profunda de uma pose (o GSAP anima objetos simples) */
export const clonePose = (pose: ArmPose): ArmPose => ({
  p0: { ...pose.p0 },
  p1: { ...pose.p1 },
  p2: { ...pose.p2 },
  p3: { ...pose.p3 },
});
