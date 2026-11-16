import { Container, Graphics, Text, TextStyle } from 'pixi.js';

// Parchment ramp, part of the scene palette (DESIGN §5.3). Deliberately not
// any `--status-*` token: the ribbon is rust, never status red.
export const PARCHMENT = {
  light: 0xdccfa8,
  mid: 0xc8b588,
  dark: 0xa38b5c,
  edge: 0x6e5a3a,
  ink: 0x2b2118,
  inkSoft: 0x4f4030,
  ribbon: 0x8a4b2d,
  ribbonDark: 0x5e3320,
};

const PX = 2; // one art pixel
const ROLL = 10; // height of each rolled end
const PAD = 12;

export interface ScrollAction {
  label: string;
  onTap: () => void;
  /** Filled (primary) or outlined button. */
  primary?: boolean;
}

export interface ScrollOptions {
  width: number;
  title: string;
  body?: string;
  actions?: ScrollAction[];
}

const BUTTON_H = 28; // minimum click target (DESIGN §8)
const BUTTON_GAP = 8;

function createButton(action: ScrollAction, width: number): Container {
  const btn = new Container();
  const g = new Graphics();
  g.rect(0, 0, width, BUTTON_H).fill(action.primary ? PARCHMENT.ink : PARCHMENT.light);
  g.rect(0, 0, width, PX).fill(PARCHMENT.edge);
  g.rect(0, BUTTON_H - PX, width, PX).fill(PARCHMENT.edge);
  g.rect(0, 0, PX, BUTTON_H).fill(PARCHMENT.edge);
  g.rect(width - PX, 0, PX, BUTTON_H).fill(PARCHMENT.edge);
  const label = new Text({
    text: action.label,
    style: new TextStyle({
      fontFamily: 'Atkinson Hyperlegible',
      fontSize: 14,
      fontWeight: '700',
      fill: action.primary ? PARCHMENT.light : PARCHMENT.ink,
    }),
  });
  label.anchor.set(0.5);
  label.position.set(width / 2, BUTTON_H / 2);
  btn.addChild(g, label);
  btn.eventMode = 'static';
  btn.cursor = 'pointer';
  btn.accessible = true;
  btn.accessibleTitle = action.label;
  btn.on('pointertap', action.onTap);
  btn.on('pointerover', () => (g.alpha = 0.85));
  btn.on('pointerout', () => (g.alpha = 1));
  return btn;
}

export function createScroll({ width, title, body, actions }: ScrollOptions): Container {
  const root = new Container();
  const textWidth = width - 2 * PAD - 2 * PX * 3;

  const titleText = new Text({
    text: title,
    style: new TextStyle({
      fontFamily: 'Pixelify Sans',
      fontSize: 16,
      fill: PARCHMENT.ink,
      wordWrap: true,
      wordWrapWidth: textWidth,
    }),
  });
  titleText.position.set(PAD + PX * 3, ROLL + PAD - 2);
  let y = titleText.y + titleText.height;

  const bodyText = body
    ? new Text({
        text: body,
        style: new TextStyle({
          fontFamily: 'Atkinson Hyperlegible',
          fontSize: 13,
          lineHeight: 18,
          fill: PARCHMENT.inkSoft,
          wordWrap: true,
          wordWrapWidth: textWidth,
        }),
      })
    : null;
  if (bodyText) {
    bodyText.position.set(PAD + PX * 3, y + 4);
    y = bodyText.y + bodyText.height;
  }

  const buttons: Container[] = [];
  if (actions && actions.length > 0) {
    const bw = Math.floor((textWidth - BUTTON_GAP * (actions.length - 1)) / actions.length);
    y += 10;
    actions.forEach((action, i) => {
      const btn = createButton(action, bw);
      btn.position.set(PAD + PX * 3 + i * (bw + BUTTON_GAP), y);
      buttons.push(btn);
    });
    y += BUTTON_H;
  }

  const height = Math.max(Math.round(y + PAD + ROLL), 2 * ROLL + 24);
  const g = new Graphics();

  // Sheet with stepped (notched) corners.
  g.rect(PX * 3, ROLL, width - PX * 6, height - 2 * ROLL).fill(PARCHMENT.light);
  g.rect(PX * 3, ROLL, width - PX * 6, PX).fill(PARCHMENT.mid);
  g.rect(PX * 3, height - ROLL - PX, width - PX * 6, PX).fill(PARCHMENT.mid);
  g.rect(PX * 3, ROLL, PX, height - 2 * ROLL).fill(PARCHMENT.mid);
  g.rect(width - PX * 4, ROLL, PX, height - 2 * ROLL).fill(PARCHMENT.mid);

  // Rolled top and bottom with a lit face, a shadow row and dark end caps.
  for (const top of [0, height - ROLL]) {
    g.rect(0, top, width, ROLL).fill(PARCHMENT.mid);
    g.rect(PX, top + PX, width - 2 * PX, PX * 2).fill(PARCHMENT.light);
    g.rect(PX, top + ROLL - PX * 2, width - 2 * PX, PX).fill(PARCHMENT.dark);
    g.rect(0, top, PX * 3, ROLL).fill(PARCHMENT.edge);
    g.rect(width - PX * 3, top, PX * 3, ROLL).fill(PARCHMENT.edge);
  }

  // Ribbon bow on the top right roll.
  const bx = width - PX * 12;
  g.rect(bx, 0, PX * 3, ROLL + PX * 2).fill(PARCHMENT.ribbon);
  g.rect(bx + PX * 3, 0, PX * 3, ROLL + PX * 2).fill(PARCHMENT.ribbonDark);
  g.rect(bx - PX, ROLL + PX * 2, PX * 3, PX * 3).fill(PARCHMENT.ribbon);
  g.rect(bx + PX * 3, ROLL + PX * 2, PX * 3, PX * 3).fill(PARCHMENT.ribbonDark);

  root.addChild(g, titleText);
  if (bodyText) root.addChild(bodyText);
  buttons.forEach((b) => root.addChild(b));
  return root;
}
