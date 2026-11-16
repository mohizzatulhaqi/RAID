import React, { useEffect, useRef } from 'react';
import { Application, Container, Graphics, Text, TextStyle } from 'pixi.js';
import { Character, PresentationState } from '../types';

interface WorldViewProps {
  characters: Character[];
  activeState: PresentationState;
  bossHp: number;
  bossName: string;
}

export const WorldView: React.FC<WorldViewProps> = ({
  characters,
  activeState,
  bossHp,
  bossName,
}) => {
  const containerRef = useRef<HTMLDivElement>(null);
  const appRef = useRef<Application | null>(null);

  useEffect(() => {
    let isCancelled = false;
    const currentContainer = containerRef.current;
    if (!currentContainer) return;

    const initPixi = async () => {
      const app = new Application();
      await app.init({
        resizeTo: currentContainer,
        backgroundColor: 0x0d1017,
        antialias: true,
        resolution: window.devicePixelRatio || 1,
        autoDensity: true,
      });

      if (isCancelled) {
        app.destroy(true, { children: true });
        return;
      }

      appRef.current = app;
      currentContainer.appendChild(app.canvas);

      // World Stage Root
      const world = new Container();
      app.stage.addChild(world);

      // 1. Draw Guild Hub Floor Tiles
      const floor = new Graphics();
      const tileSize = 48;
      const cols = 24;
      const rows = 16;
      const startX = (app.screen.width - cols * tileSize) / 2;
      const startY = 80;

      for (let r = 0; r < rows; r++) {
        for (let c = 0; c < cols; c++) {
          const isAlt = (r + c) % 2 === 0;
          floor.rect(startX + c * tileSize, startY + r * tileSize, tileSize, tileSize);
          floor.fill(isAlt ? 0x141923 : 0x11151e);
          floor.stroke({ width: 1, color: 0x1c2433 });
        }
      }
      world.addChild(floor);

      // 2. Center Mystic Guild Crest
      const centerX = app.screen.width / 2;
      const centerY = startY + (rows * tileSize) / 2 + 20;

      const crest = new Graphics();
      crest.circle(centerX, centerY, 80);
      crest.fill({ color: 0x1f2738, alpha: 0.6 });
      crest.stroke({ width: 2, color: 0xf59e0b, alpha: 0.5 });
      crest.circle(centerX, centerY, 60);
      crest.stroke({ width: 1, color: 0x06b6d4, alpha: 0.4 });
      world.addChild(crest);

      // 3. Dungeon Portal / Boss Gate at Top
      const portal = new Graphics();
      const portalY = startY + 10;
      portal.roundRect(centerX - 90, portalY - 40, 180, 50, 12);
      portal.fill({ color: 0x221122, alpha: 0.8 });
      portal.stroke({ width: 2, color: 0xef4444, alpha: 0.8 });

      // Portal energy glow
      portal.circle(centerX, portalY - 15, 20);
      portal.fill({ color: 0xef4444, alpha: 0.4 });
      world.addChild(portal);

      const portalText = new Text({
        text: `BOSS: ${bossName} (${bossHp}%)`,
        style: new TextStyle({
          fontFamily: 'Cinzel, serif',
          fontSize: 12,
          fontWeight: 'bold',
          fill: 0xfca5a5,
        }),
      });
      portalText.anchor.set(0.5);
      portalText.position.set(centerX, portalY - 15);
      world.addChild(portalText);

      // 4. Render Characters in the Hub
      const charContainers: Container[] = [];
      const charPositions = [
        { x: centerX - 120, y: centerY + 20, color: 0xf59e0b, label: 'Warrior' },
        { x: centerX, y: centerY + 40, color: 0x06b6d4, label: 'Mage' },
        { x: centerX + 120, y: centerY + 20, color: 0x10b981, label: 'Rogue' },
      ];

      characters.slice(0, 3).forEach((char, idx) => {
        const pos = charPositions[idx] || charPositions[0];
        const charGroup = new Container();
        charGroup.position.set(pos.x, pos.y);

        // Shadow
        const shadow = new Graphics();
        shadow.ellipse(0, 16, 22, 8);
        shadow.fill({ color: 0x000000, alpha: 0.5 });
        charGroup.addChild(shadow);

        // Body Avatar (Stylized RPG Token)
        const body = new Graphics();
        body.circle(0, 0, 18);
        body.fill(pos.color);
        body.stroke({ width: 3, color: 0xffffff });
        charGroup.addChild(body);

        // Character Name Label
        const nameText = new Text({
          text: char.display_name,
          style: new TextStyle({
            fontFamily: 'Inter, sans-serif',
            fontSize: 11,
            fontWeight: '600',
            fill: 0xffffff,
          }),
        });
        nameText.anchor.set(0.5);
        nameText.position.set(0, 30);
        charGroup.addChild(nameText);

        // Class/Role badge
        const roleText = new Text({
          text: `${char.class.toUpperCase()} • ${char.party_role.toUpperCase()}`,
          style: new TextStyle({
            fontFamily: 'Inter, sans-serif',
            fontSize: 9,
            fill: 0x94a3b8,
          }),
        });
        roleText.anchor.set(0.5);
        roleText.position.set(0, 42);
        charGroup.addChild(roleText);

        // Status speech bubble
        const bubble = new Graphics();
        bubble.roundRect(-40, -38, 80, 20, 6);
        bubble.fill({ color: 0x1e293b, alpha: 0.9 });
        bubble.stroke({ width: 1, color: pos.color, alpha: 0.7 });
        charGroup.addChild(bubble);

        const statusLabel = new Text({
          text: char.state.toUpperCase().replace('_', ' '),
          style: new TextStyle({
            fontFamily: 'Inter, sans-serif',
            fontSize: 9,
            fontWeight: 'bold',
            fill: pos.color,
          }),
        });
        statusLabel.anchor.set(0.5);
        statusLabel.position.set(0, -28);
        charGroup.addChild(statusLabel);

        world.addChild(charGroup);
        charContainers.push(charGroup);
      });

      // Subtle breathing / idle animation loop
      let tickerCount = 0;
      app.ticker.add(() => {
        tickerCount += 0.04;
        charContainers.forEach((cg, i) => {
          cg.position.y += Math.sin(tickerCount + i * 1.5) * 0.2;
        });
      });
    };

    initPixi();

    return () => {
      isCancelled = true;
      if (appRef.current) {
        appRef.current.destroy(true, { children: true });
        appRef.current = null;
      }
    };
  }, [characters, bossHp, bossName, activeState]);

  return (
    <div
      ref={containerRef}
      style={{
        width: '100%',
        height: '100%',
        position: 'relative',
        overflow: 'hidden',
        borderRadius: '12px',
      }}
    />
  );
};
