import { gsap } from 'gsap';
import { sendBack, toRest, type Scene, type SceneHandle } from './motion';
import { ARM_NAMES, createOctapus, type Octapus } from './rig';

/**
 * Um palco com um polvo: toca uma cena por vez. Trocar de cena guarda os
 * objetos, leva os tentáculos ao repouso e só então monta a próxima — a
 * transição de ~0,35 s que liga uma fase do turno à seguinte.
 *
 * Fora da tela (ou com a janela oculta) o palco fica inativo: as animações
 * pausam e o rig para de redesenhar. Com movimento reduzido, a cena para numa
 * pose (`stillAt`) — o pulso de opacidade é do CSS.
 */
export class Stage {
  readonly rig: Octapus;
  private ctx: gsap.Context | null = null;
  private handle: SceneHandle | null = null;
  private transition: gsap.core.Timeline | null = null;
  private reduced = false;
  private active = true;
  scene: Scene | null = null;

  constructor(readonly container: HTMLElement) {
    this.rig = createOctapus(container);
  }

  setReduced(reduced: boolean) {
    if (reduced === this.reduced) return;
    this.reduced = reduced;
    if (this.scene) this.play(this.scene, { transition: false });
  }

  setActive(active: boolean) {
    if (active === this.active) return;
    this.active = active;
    if (this.reduced) return;
    for (const item of this.topLevel()) {
      if (active) item.resume();
      else item.pause();
    }
    if (active) this.transition?.resume();
    else this.transition?.pause();
    this.rig.setLive(active);
  }

  /** Monta e toca a cena; com `transition`, passa pelo repouso antes */
  play(scene: Scene, { transition = true } = {}) {
    this.transition?.kill();
    this.transition = null;

    const start = () => {
      this.transition = null;
      this.clear();
      this.scene = scene;
      this.ctx = gsap.context(() => {
        this.handle = scene.build(this.rig);
        if (!scene.loop) this.handle.timeline.repeat(0);
      });
      if (this.reduced) this.freeze();
      else if (!this.active) {
        for (const item of this.topLevel()) item.pause();
        this.rig.setLive(false);
      } else this.rig.setLive(true);
    };

    if (!transition || !this.scene || this.reduced || !this.active) {
      start();
      return;
    }
    // Sai: os objetos somem e o polvo volta ao repouso
    const props = [
      this.rig.back,
      this.rig.front,
      this.rig.cutFront,
      ...ARM_NAMES.flatMap(n => [this.rig.arms[n].hand, this.rig.arms[n].cut]),
    ];
    for (const item of this.topLevel()) item.pause();
    this.transition = gsap
      .timeline({ onComplete: start })
      .to(props, { opacity: 0, duration: 0.15, ease: 'power1.in' })
      .add(toRest(this.rig, 0.35), '<');
  }

  /** As animações de primeiro nível da cena (as de dentro seguem a delas) */
  private topLevel(): gsap.core.Animation[] {
    const all = (this.ctx as unknown as { data: unknown[] } | null)?.data ?? [];
    return all.filter(
      (item): item is gsap.core.Animation =>
        item instanceof gsap.core.Animation &&
        item.parent === gsap.globalTimeline,
    );
  }

  /**
   * Movimento reduzido: a pose parada da cena, sem nenhum loop. Loops param
   * no começo, entradas vão até o fim, a principal para em `stillAt`.
   */
  private freeze() {
    if (!this.handle) return;
    const main = this.handle.timeline;
    for (const item of this.topLevel()) {
      if (item === main) continue;
      if (item.repeat() === -1) item.pause(0);
      else item.progress(1).pause();
    }
    main.pause(main.duration() * this.handle.stillAt);
    this.rig.render();
    this.rig.setLive(false);
  }

  private clear() {
    this.ctx?.revert();
    this.ctx = null;
    this.handle = null;
    const { rig } = this;
    for (const layer of [
      rig.back,
      rig.front,
      rig.cutFront,
      ...ARM_NAMES.flatMap(n => [rig.arms[n].hand, rig.arms[n].cut]),
    ]) {
      layer.replaceChildren();
      gsap.set(layer, { clearProps: 'opacity' });
    }
    for (const name of ARM_NAMES) {
      sendBack(rig, name);
      rig.arms[name].grip = false;
      rig.arms[name].followAngle = false;
    }
    rig.render();
  }

  destroy() {
    this.transition?.kill();
    this.transition = null;
    this.clear();
    this.rig.destroy();
  }
}
