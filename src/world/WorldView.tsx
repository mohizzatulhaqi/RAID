import React, { useEffect, useRef, useState } from 'react';
import { Application, Container, Text, TextStyle, TextureStyle } from 'pixi.js';
import { Viewport } from 'pixi-viewport';
import { invoke } from '@tauri-apps/api/core';
import { BossHp, Character } from '../types';
import { createScroll } from './scroll';
import { createBoss, createMap, createToken, SCENE } from './scene';
import { AREAS, AreaId, MAX_ZOOM, MIN_ZOOM, TILE, WORLD_H, WORLD_W, areaCenter, spots } from './layout';
import { formatHp, sceneLabel, statusFromPresentation } from '../state/status';
import { statusLabel } from '../panels/StatusChip';

interface WorldViewProps {
  projectId: string | null;
  characters: Character[];
  /** Characters whose task waits for or failed checks; they stand in the arena. */
  arenaCharacterIds: string[];
  bossName: string;
  bossHp: BossHp | null;
  questTitle: string | null;
  approvalText: string | null;
  onApprove: () => void;
  onDeny: () => void;
  reduceMotion: boolean;
}

interface Camera {
  x: number;
  y: number;
  zoom: number;
}

const cap = (s: string) => s.charAt(0).toUpperCase() + s.slice(1);

const label = (text: string, size: number, color: number) =>
  new Text({ text, style: new TextStyle({ fontFamily: 'Atkinson Hyperlegible', fontSize: size, fill: color }) });

export const WorldView: React.FC<WorldViewProps> = ({
  projectId,
  characters,
  arenaCharacterIds,
  bossName,
  bossHp,
  questTitle,
  approvalText,
  onApprove,
  onDeny,
  reduceMotion,
}) => {
  const hostRef = useRef<HTMLDivElement>(null);
  const appRef = useRef<Application | null>(null);
  const viewportRef = useRef<Viewport | null>(null);
  const charLayerRef = useRef<Container | null>(null);
  const worldScrollsRef = useRef<Container | null>(null);
  const hudRef = useRef<Container | null>(null);
  const bossLayerRef = useRef<Container | null>(null);
  const bobbersRef = useRef<{ node: Container; baseY: number }[]>([]);
  const reduceRef = useRef(reduceMotion);
  const approveRef = useRef(onApprove);
  const denyRef = useRef(onDeny);
  const projectRef = useRef(projectId);
  const liveRef = useRef<Set<string>>(new Set());
  const [ready, setReady] = useState(false);
  const [fontsReady, setFontsReady] = useState(false);
  const [zoom, setZoom] = useState(1);

  useEffect(() => {
    approveRef.current = onApprove;
    denyRef.current = onDeny;
    reduceRef.current = reduceMotion;
    projectRef.current = projectId;
  }, [onApprove, onDeny, reduceMotion, projectId]);

  useEffect(() => {
    let alive = true;
    Promise.all([document.fonts.load('16px "Pixelify Sans"'), document.fonts.load('13px "Atkinson Hyperlegible"')])
      .catch(() => undefined)
      .then(() => alive && setFontsReady(true));
    return () => {
      alive = false;
    };
  }, []);

  useEffect(() => {
    const host = hostRef.current;
    if (!host) return;
    let cancelled = false;
    let onVisibility: (() => void) | undefined;
    let saveTimer: ReturnType<typeof setTimeout> | undefined;

    (async () => {
      TextureStyle.defaultOptions.scaleMode = 'nearest';
      const app = new Application();
      await app.init({
        resizeTo: host,
        backgroundAlpha: 0,
        antialias: false,
        roundPixels: true,
        resolution: window.devicePixelRatio || 1,
        autoDensity: true,
      });
      if (cancelled) {
        app.destroy(true, { children: true });
        return;
      }
      appRef.current = app;
      host.appendChild(app.canvas);

      const viewport = new Viewport({
        screenWidth: app.screen.width,
        screenHeight: app.screen.height,
        worldWidth: WORLD_W,
        worldHeight: WORLD_H,
        events: app.renderer.events,
      });
      viewport
        .drag()
        .pinch()
        .wheel({ smooth: 4 })
        .clampZoom({ minScale: MIN_ZOOM, maxScale: MAX_ZOOM })
        .clamp({ direction: 'all', underflow: 'center' });
      app.stage.addChild(viewport);
      viewportRef.current = viewport;

      viewport.addChild(createMap());
      const bossLayer = new Container();
      const worldScrolls = new Container();
      const charLayer = new Container();
      viewport.addChild(bossLayer, worldScrolls, charLayer);
      bossLayerRef.current = bossLayer;
      worldScrollsRef.current = worldScrolls;
      charLayerRef.current = charLayer;

      // Screen-fixed layer: an approval must be visible wherever the camera is.
      const hud = new Container();
      app.stage.addChild(hud);
      hudRef.current = hud;

      // Integer zoom keeps pixels sharp (DESIGN §5.2).
      viewport.on('zoomed-end', () => {
        const snapped = Math.min(MAX_ZOOM, Math.max(MIN_ZOOM, Math.round(viewport.scale.x)));
        if (snapped !== viewport.scale.x) viewport.setZoom(snapped, true);
        setZoom(snapped);
      });
      viewport.on('moved-end', () => {
        clearTimeout(saveTimer);
        saveTimer = setTimeout(() => {
          const pid = projectRef.current;
          if (!pid) return;
          const camera: Camera = { x: viewport.center.x, y: viewport.center.y, zoom: viewport.scale.x };
          invoke('save_camera', { projectId: pid, camera }).catch(() => undefined);
        }, 600);
      });

      app.renderer.on('resize', (w: number, h: number) => viewport.resize(w, h, WORLD_W, WORLD_H));

      viewport.setZoom(1, true);
      viewport.moveCenter(areaCenter('hub').x, areaCenter('hub').y);

      app.ticker.add((ticker) => {
        if (reduceRef.current) return;
        const t = ticker.lastTime / 600;
        bobbersRef.current.forEach((b, i) => {
          b.node.y = Math.round(b.baseY + Math.sin(t + i * 1.5) * 2);
        });
      });

      onVisibility = () => (document.hidden ? app.ticker.stop() : app.ticker.start());
      document.addEventListener('visibilitychange', onVisibility);
      setReady(true);
    })();

    return () => {
      cancelled = true;
      clearTimeout(saveTimer);
      if (onVisibility) document.removeEventListener('visibilitychange', onVisibility);
      if (appRef.current) {
        appRef.current.destroy(true, { children: true });
        appRef.current = null;
      }
      viewportRef.current = null;
      charLayerRef.current = null;
      worldScrollsRef.current = null;
      hudRef.current = null;
      bossLayerRef.current = null;
      bobbersRef.current = [];
      setReady(false);
    };
  }, []);

  const moveTo = (x: number, y: number, scale?: number) => {
    const viewport = viewportRef.current;
    if (!viewport) return;
    if (reduceRef.current) {
      if (scale) viewport.setZoom(scale, true);
      viewport.moveCenter(x, y);
    } else {
      viewport.animate({ position: { x, y }, scale, time: 400, ease: 'easeInOutSine' });
    }
  };

  const goTo = (id: AreaId) => {
    const c = areaCenter(id);
    moveTo(c.x, c.y);
  };

  const zoomBy = (step: number) => {
    const viewport = viewportRef.current;
    if (!viewport) return;
    const next = Math.min(MAX_ZOOM, Math.max(MIN_ZOOM, Math.round(viewport.scale.x) + step));
    viewport.setZoom(next, true);
    setZoom(next);
  };

  // Restore the saved camera of this project.
  useEffect(() => {
    if (!ready || !projectId) return;
    let stale = false;
    invoke<Camera | null>('get_camera', { projectId })
      .then((cam) => {
        const viewport = viewportRef.current;
        if (stale || !viewport || !cam || (cam.x === 0 && cam.y === 0)) return;
        const z = Math.min(MAX_ZOOM, Math.max(MIN_ZOOM, Math.round(cam.zoom)));
        viewport.setZoom(z, true);
        viewport.moveCenter(cam.x, cam.y);
        setZoom(z);
      })
      .catch(() => undefined);
    return () => {
      stale = true;
    };
  }, [ready, projectId]);

  // Boss in the arena, with its scroll. Text and HP come from the backend.
  useEffect(() => {
    const layer = bossLayerRef.current;
    if (!ready || !fontsReady || !layer) return;
    layer.removeChildren().forEach((c) => c.destroy({ children: true }));
    const arena = AREAS.arena;
    const center = areaCenter('arena');
    const boss = createBoss(bossHp?.defeated ?? false);
    boss.position.set(center.x, center.y + TILE);
    layer.addChild(boss);
    const plaque = createScroll({
      width: 300,
      title: bossName,
      body: bossHp
        ? bossHp.defeated
          ? 'Boss defeated: build, test and lint passed.'
          : `HP ${formatHp(bossHp.remaining_bp)}. ${bossHp.label}`
        : 'No encounter yet. Select a quest.',
    });
    plaque.position.set(Math.round(center.x - 150), arena.y + 24);
    layer.addChild(plaque);
  }, [ready, fontsReady, bossName, bossHp]);

  // Quest board scroll in the hub, approval scroll fixed on screen.
  useEffect(() => {
    const app = appRef.current;
    const scrolls = worldScrollsRef.current;
    const hud = hudRef.current;
    if (!ready || !fontsReady || !app || !scrolls || !hud) return;
    scrolls.removeChildren().forEach((c) => c.destroy({ children: true }));
    hud.removeChildren().forEach((c) => c.destroy({ children: true }));

    const quest = createScroll({
      width: 280,
      title: 'Quest board',
      body: questTitle ?? 'No quest selected. Create a quest to post it here.',
    });
    quest.position.set(AREAS.hub.x + 32, AREAS.hub.y + 32);
    scrolls.addChild(quest);

    if (approvalText) {
      const ask = createScroll({
        width: 340,
        title: 'Approval needed',
        body: approvalText,
        actions: [
          { label: 'Deny', onTap: () => denyRef.current() },
          { label: 'Approve', onTap: () => approveRef.current(), primary: true },
        ],
      });
      ask.position.set(Math.round(app.screen.width / 2 - 170), 160);
      hud.addChild(ask);
    }
  }, [ready, fontsReady, questTitle, approvalText]);

  // Characters: hub when idle, dungeon with a live session, arena while their
  // task waits on checks. Positions follow backend state only.
  useEffect(() => {
    const layer = charLayerRef.current;
    if (!ready || !layer) return;
    layer.removeChildren().forEach((c) => c.destroy({ children: true }));
    bobbersRef.current = [];

    const where = (c: Character): AreaId =>
      c.active_session_id ? 'dungeon' : arenaCharacterIds.includes(c.id) ? 'arena' : 'hub';
    const groups: Record<AreaId, Character[]> = { hub: [], dungeon: [], arena: [] };
    characters.forEach((c) => groups[where(c)].push(c));

    (Object.keys(groups) as AreaId[]).forEach((area) => {
      const places = spots(area, groups[area].length, area === 'arena' ? 4 * TILE : 2 * TILE);
      groups[area].forEach((char, i) => {
        const spot = places[i];
        const group = new Container();
        group.position.set(spot.x, spot.y);
        group.addChild(createToken(char.class));

        const name = label(char.display_name, 13, SCENE.bone);
        name.anchor.set(0.5);
        name.position.set(0, 30);
        const meta = label(`${cap(char.class)}, ${cap(char.party_role)}`, 12, SCENE.stone);
        meta.anchor.set(0.5);
        meta.position.set(0, 46);
        const state = label(sceneLabel(char) ?? statusLabel(statusFromPresentation(char.state)), 12, SCENE.bone);
        state.anchor.set(0.5);
        state.position.set(0, -44);
        group.addChild(name, meta, state);

        layer.addChild(group);
        bobbersRef.current.push({ node: group, baseY: spot.y });
      });
    });

    // Follow a character into the dungeon once, when its session starts.
    const live = new Set(characters.filter((c) => c.active_session_id).map((c) => c.id));
    const started = [...live].some((id) => !liveRef.current.has(id));
    liveRef.current = live;
    if (started) {
      const c = areaCenter('dungeon');
      const viewport = viewportRef.current;
      if (viewport) {
        if (reduceRef.current) viewport.moveCenter(c.x, c.y);
        else viewport.animate({ position: c, time: 400, ease: 'easeInOutSine' });
      }
    }
  }, [characters, arenaCharacterIds, ready]);

  return (
    <div style={{ width: '100%', height: '100%', position: 'relative' }}>
      <div ref={hostRef} style={{ width: '100%', height: '100%' }} />
      <nav className="world-nav" aria-label="World camera">
        <button className="btn" onClick={() => goTo('hub')}>
          Guild Hub
        </button>
        <button className="btn" onClick={() => goTo('dungeon')}>
          Dungeon
        </button>
        <button className="btn" onClick={() => goTo('arena')}>
          Boss arena
        </button>
        <span className="world-zoom">
          <button className="btn" aria-label="Zoom out" disabled={zoom <= MIN_ZOOM} onClick={() => zoomBy(-1)}>
            -
          </button>
          <span className="num">{zoom}x</span>
          <button className="btn" aria-label="Zoom in" disabled={zoom >= MAX_ZOOM} onClick={() => zoomBy(1)}>
            +
          </button>
        </span>
      </nav>
    </div>
  );
};
