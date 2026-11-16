import React, { useEffect, useRef, useState } from 'react';
import { Application, Container, Graphics, Text, TextStyle, TextureStyle } from 'pixi.js';
import { BossHp, Character } from '../types';
import { createScroll } from './scroll';
import { formatHp } from '../state/status';
import { sceneLabel, statusFromPresentation } from '../state/status';
import { statusLabel } from '../panels/StatusChip';

interface WorldViewProps {
  characters: Character[];
  bossName: string;
  /** Boss HP and its text label, both computed by the backend. */
  bossHp: BossHp | null;
  questTitle: string | null;
  /** Text of the pending approval request, if any. */
  approvalText: string | null;
  onApprove: () => void;
  onDeny: () => void;
  reduceMotion: boolean;
}

// Scene ramp (DESIGN §5.3): stone, moss, rust, bone. Deliberately not the
// status colors, so decoration never reads as status.
const SCENE = {
  floorA: 0x141923,
  floorB: 0x11151e,
  grout: 0x1c2433,
  rust: 0x8a4b2d,
  moss: 0x4d6b4a,
  bone: 0xd8d2c2,
  stone: 0x6b7a90,
};
const CLASS_TINT: Record<string, number> = {
  warrior: SCENE.rust,
  mage: SCENE.stone,
  rogue: SCENE.moss,
};

const cap = (s: string) => s.charAt(0).toUpperCase() + s.slice(1);

export const WorldView: React.FC<WorldViewProps> = ({
  characters,
  bossName,
  bossHp,
  questTitle,
  approvalText,
  onApprove,
  onDeny,
  reduceMotion,
}) => {
  // Latest handlers, so the scroll is not redrawn when only a callback changes.
  const approveRef = useRef(onApprove);
  const denyRef = useRef(onDeny);
  useEffect(() => {
    approveRef.current = onApprove;
    denyRef.current = onDeny;
  }, [onApprove, onDeny]);
  const hostRef = useRef<HTMLDivElement>(null);
  const appRef = useRef<Application | null>(null);
  const charLayerRef = useRef<Container | null>(null);
  const scrollLayerRef = useRef<Container | null>(null);
  const bobbersRef = useRef<{ node: Container; baseY: number }[]>([]);
  const reduceRef = useRef(reduceMotion);
  const [ready, setReady] = useState(false);
  const [fontsReady, setFontsReady] = useState(false);

  useEffect(() => {
    let alive = true;
    Promise.all([
      document.fonts.load('16px "Pixelify Sans"'),
      document.fonts.load('13px "Atkinson Hyperlegible"'),
    ])
      .catch(() => undefined)
      .then(() => alive && setFontsReady(true));
    return () => {
      alive = false;
    };
  }, []);

  useEffect(() => {
    reduceRef.current = reduceMotion;
  }, [reduceMotion]);

  useEffect(() => {
    const host = hostRef.current;
    if (!host) return;
    let cancelled = false;
    let onVisibility: (() => void) | undefined;

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

      const world = new Container();
      app.stage.addChild(world);

      const tile = 48;
      const cols = 24;
      const rows = 16;
      const startX = (app.screen.width - cols * tile) / 2;
      const startY = 100;
      const floor = new Graphics();
      for (let r = 0; r < rows; r++) {
        for (let c = 0; c < cols; c++) {
          floor.rect(startX + c * tile, startY + r * tile, tile, tile);
          floor.fill((r + c) % 2 === 0 ? SCENE.floorA : SCENE.floorB);
          floor.stroke({ width: 1, color: SCENE.grout });
        }
      }
      world.addChild(floor);

      const charLayer = new Container();
      world.addChild(charLayer);
      charLayerRef.current = charLayer;

      const scrollLayer = new Container();
      world.addChild(scrollLayer);
      scrollLayerRef.current = scrollLayer;

      app.ticker.add((ticker) => {
        if (reduceRef.current) return;
        const t = ticker.lastTime / 600;
        bobbersRef.current.forEach((b, i) => {
          b.node.y = Math.round(b.baseY + Math.sin(t + i * 1.5) * 2);
        });
      });

      // Pause rendering while the window is hidden (DESIGN §3.5).
      onVisibility = () => (document.hidden ? app.ticker.stop() : app.ticker.start());
      document.addEventListener('visibilitychange', onVisibility);

      setReady(true);
    })();

    return () => {
      cancelled = true;
      if (onVisibility) document.removeEventListener('visibilitychange', onVisibility);
      if (appRef.current) {
        appRef.current.destroy(true, { children: true });
        appRef.current = null;
      }
      charLayerRef.current = null;
      scrollLayerRef.current = null;
      bobbersRef.current = [];
      setReady(false);
    };
  }, []);

  // Scrolls: the quest board, the boss plaque and a pending approval. They show
  // only text from backend data; progress itself is drawn by the boss bar.
  useEffect(() => {
    const app = appRef.current;
    const layer = scrollLayerRef.current;
    if (!ready || !fontsReady || !app || !layer) return;

    layer.removeChildren().forEach((c) => c.destroy({ children: true }));
    const cx = app.screen.width / 2;
    const top = 24;
    const w = 260;

    const quest = createScroll({
      width: w,
      title: 'Quest board',
      body: questTitle ?? 'No quest selected. Create a quest to post it here.',
    });
    quest.position.set(Math.round(cx - w - 24), top);
    layer.addChild(quest);

    const boss = createScroll({
      width: w,
      title: bossName,
      body: bossHp ? `HP ${formatHp(bossHp.remaining_bp)}. ${bossHp.label}` : 'No encounter yet.',
    });
    boss.position.set(Math.round(cx + 24), top);
    layer.addChild(boss);

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
      ask.position.set(Math.round(cx - 170), top + 150);
      layer.addChild(ask);
    }
  }, [ready, fontsReady, bossName, bossHp, questTitle, approvalText]);

  useEffect(() => {
    const app = appRef.current;
    const layer = charLayerRef.current;
    if (!ready || !app || !layer) return;

    layer.removeChildren().forEach((c) => c.destroy({ children: true }));
    bobbersRef.current = [];

    const cx = app.screen.width / 2;
    const cy = 100 + (16 * 48) / 2 + 20;
    const slots = [
      { x: cx - 120, y: cy + 20 },
      { x: cx, y: cy + 40 },
      { x: cx + 120, y: cy + 20 },
    ];

    // A character with a live session walks up to the dungeon gate.
    const gate = { x: cx, y: cy - 90 };
    characters.slice(0, slots.length).forEach((char, i) => {
      const slot = char.active_session_id ? gate : slots[i];
      const group = new Container();
      group.position.set(slot.x, slot.y);

      const body = new Graphics();
      body.circle(0, 0, 16);
      body.fill(CLASS_TINT[char.class] ?? SCENE.stone);
      body.stroke({ width: 2, color: SCENE.bone });
      group.addChild(body);

      const name = new Text({
        text: char.display_name,
        style: new TextStyle({ fontFamily: 'Atkinson Hyperlegible', fontSize: 13, fill: SCENE.bone }),
      });
      name.anchor.set(0.5);
      name.position.set(0, 30);
      group.addChild(name);

      const meta = new Text({
        text: `${cap(char.class)}, ${cap(char.party_role)}`,
        style: new TextStyle({ fontFamily: 'Atkinson Hyperlegible', fontSize: 12, fill: SCENE.stone }),
      });
      meta.anchor.set(0.5);
      meta.position.set(0, 46);
      group.addChild(meta);

      // State text comes from the character's stored state, not an invented one.
      const state = new Text({
        text: sceneLabel(char) ?? statusLabel(statusFromPresentation(char.state)),
        style: new TextStyle({ fontFamily: 'Atkinson Hyperlegible', fontSize: 12, fill: SCENE.bone }),
      });
      state.anchor.set(0.5);
      state.position.set(0, -30);
      group.addChild(state);

      layer.addChild(group);
      bobbersRef.current.push({ node: group, baseY: slot.y });
    });
  }, [characters, ready]);

  return <div ref={hostRef} style={{ width: '100%', height: '100%' }} />;
};
