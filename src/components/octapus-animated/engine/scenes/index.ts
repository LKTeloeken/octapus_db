import type { OctapusSceneId } from '../../scene-id';
import type { Scene } from '../motion';
import { clipboardScene } from './clipboard';
import { computerScene } from './computer';
import { hourglassScene } from './hourglass';
import { magnifierScene } from './magnifier';
import { celebrateScene, scratchScene } from './oneshots';
import { tablesScene } from './tables';
import { thinking } from './thinking';

export const SCENES: Record<OctapusSceneId, Scene> = {
  thinking,
  magnifier: magnifierScene,
  computer: computerScene,
  clipboard: clipboardScene,
  tables: tablesScene,
  hourglass: hourglassScene,
  scratch: scratchScene,
  celebrate: celebrateScene,
};
