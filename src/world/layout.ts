export const TILE = 48;

export interface Area {
  id: AreaId;
  label: string;
  x: number;
  y: number;
  w: number;
  h: number;
}

export type AreaId = 'hub' | 'dungeon' | 'arena';

// One map, left to right: Guild Hub, a corridor, the dungeon, a corridor, the boss arena.
export const AREAS: Record<AreaId, Area> = {
  hub: { id: 'hub', label: 'Guild Hub', x: 0, y: 0, w: 20 * TILE, h: 15 * TILE },
  dungeon: { id: 'dungeon', label: 'Dungeon', x: 26 * TILE, y: 2 * TILE, w: 16 * TILE, h: 11 * TILE },
  arena: { id: 'arena', label: 'Boss arena', x: 48 * TILE, y: 0, w: 20 * TILE, h: 15 * TILE },
};

export const CORRIDORS = [
  { x: 20 * TILE, y: 6 * TILE, w: 6 * TILE, h: 3 * TILE },
  { x: 42 * TILE, y: 6 * TILE, w: 6 * TILE, h: 3 * TILE },
];

export const WORLD_W = 68 * TILE;
export const WORLD_H = 15 * TILE;

export const MIN_ZOOM = 1;
export const MAX_ZOOM = 3;

export function areaCenter(id: AreaId): { x: number; y: number } {
  const a = AREAS[id];
  return { x: a.x + a.w / 2, y: a.y + a.h / 2 };
}

/** Spots for `count` characters inside an area, spread on one row. */
export function spots(id: AreaId, count: number, rowOffset = 2 * TILE): { x: number; y: number }[] {
  const c = areaCenter(id);
  const gap = 3 * TILE;
  const start = c.x - ((count - 1) * gap) / 2;
  return Array.from({ length: count }, (_, i) => ({ x: Math.round(start + i * gap), y: c.y + rowOffset }));
}
