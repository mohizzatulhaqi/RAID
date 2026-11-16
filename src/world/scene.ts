import { Container, Graphics, Text, TextStyle } from 'pixi.js';
import { AREAS, CORRIDORS, TILE } from './layout';

// Scene ramps (DESIGN §5.3): stone, moss, rust, bone. Never a status color.
export const SCENE = {
  floorA: 0x141923,
  floorB: 0x11151e,
  dungeonA: 0x161a20,
  dungeonB: 0x12161b,
  arenaA: 0x1a1618,
  arenaB: 0x151214,
  grout: 0x1c2433,
  wall: 0x2a3140,
  wallTop: 0x3a4356,
  rust: 0x8a4b2d,
  rustDark: 0x5e3320,
  moss: 0x4d6b4a,
  bone: 0xd8d2c2,
  stone: 0x6b7a90,
  stoneDark: 0x4a5566,
};

const PALETTE: Record<string, [number, number]> = {
  hub: [SCENE.floorA, SCENE.floorB],
  dungeon: [SCENE.dungeonA, SCENE.dungeonB],
  arena: [SCENE.arenaA, SCENE.arenaB],
};

function floor(g: Graphics, x: number, y: number, w: number, h: number, a: number, b: number) {
  for (let ty = 0; ty < h / TILE; ty++) {
    for (let tx = 0; tx < w / TILE; tx++) {
      g.rect(x + tx * TILE, y + ty * TILE, TILE, TILE)
        .fill((tx + ty) % 2 === 0 ? a : b)
        .stroke({ width: 1, color: SCENE.grout });
    }
  }
}

function walls(g: Graphics, x: number, y: number, w: number, h: number) {
  const t = 8;
  g.rect(x - t, y - t, w + 2 * t, t).fill(SCENE.wallTop);
  g.rect(x - t, y + h, w + 2 * t, t).fill(SCENE.wall);
  g.rect(x - t, y, t, h).fill(SCENE.wall);
  g.rect(x + w, y, t, h).fill(SCENE.wall);
}

/** Static floors, walls, corridors and area names. */
export function createMap(): Container {
  const root = new Container();
  const g = new Graphics();
  for (const c of CORRIDORS) floor(g, c.x, c.y, c.w, c.h, SCENE.dungeonA, SCENE.dungeonB);
  for (const area of Object.values(AREAS)) {
    const [a, b] = PALETTE[area.id];
    floor(g, area.x, area.y, area.w, area.h, a, b);
    walls(g, area.x, area.y, area.w, area.h);
  }
  // Re-open the walls where corridors join the rooms.
  for (const c of CORRIDORS) {
    g.rect(c.x - 8, c.y, 8, c.h).fill(SCENE.dungeonB);
    g.rect(c.x + c.w, c.y, 8, c.h).fill(SCENE.dungeonB);
  }
  root.addChild(g);

  for (const area of Object.values(AREAS)) {
    const label = new Text({
      text: area.label,
      style: new TextStyle({ fontFamily: 'Pixelify Sans', fontSize: 16, fill: SCENE.bone }),
    });
    label.position.set(area.x + 16, area.y + area.h - 32);
    root.addChild(label);
  }
  return root;
}

const PX = 4;

/** Boss drawn from rectangles in the rust and stone ramps. */
export function createBoss(defeated: boolean): Container {
  const root = new Container();
  const g = new Graphics();
  const px = (x: number, y: number, w: number, h: number, color: number) =>
    g.rect(x * PX, y * PX, w * PX, h * PX).fill(color);

  px(6, 0, 12, 4, SCENE.rustDark); // horns
  px(4, 2, 4, 4, SCENE.rust);
  px(16, 2, 4, 4, SCENE.rust);
  px(6, 4, 12, 10, SCENE.stone); // head
  px(8, 7, 3, 2, SCENE.bone); // eyes
  px(13, 7, 3, 2, SCENE.bone);
  px(9, 11, 6, 1, SCENE.stoneDark); // mouth
  px(2, 14, 20, 12, SCENE.stoneDark); // body
  px(4, 16, 16, 8, SCENE.stone);
  px(0, 15, 3, 9, SCENE.rust); // arms
  px(21, 15, 3, 9, SCENE.rust);
  px(5, 26, 5, 4, SCENE.rustDark); // legs
  px(14, 26, 5, 4, SCENE.rustDark);
  root.addChild(g);
  root.pivot.set(12 * PX, 30 * PX);
  if (defeated) {
    root.alpha = 0.3;
    root.rotation = Math.PI / 2;
  }
  return root;
}

/** Character token: class shown by silhouette and gear, never by status color. */
export function createToken(className: string): Graphics {
  const g = new Graphics();
  const tint = className === 'warrior' ? SCENE.rust : className === 'rogue' ? SCENE.moss : SCENE.stone;
  g.circle(0, 0, 16).fill(tint).stroke({ width: 2, color: SCENE.bone });
  if (className === 'warrior') {
    g.rect(-24, -6, 10, 16).fill(SCENE.stoneDark).stroke({ width: 2, color: SCENE.bone }); // shield
  } else if (className === 'mage') {
    g.poly([-12, -12, 12, -12, 0, -34]).fill(SCENE.stoneDark).stroke({ width: 2, color: SCENE.bone }); // hat
  } else if (className === 'rogue') {
    g.rect(16, -2, 14, 3).fill(SCENE.bone); // dagger
  }
  return g;
}
