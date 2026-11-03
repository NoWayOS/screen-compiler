// The declarations of `nowayos:kit`, written by `screen-compiler subset --dts` from
// Screen Compiler's own tables: do not edit. For an editor's TypeScript in a folder
// of screens without React's types: "jsx": "preserve" in its tsconfig.json.
// Screen Compiler checks more than these types can say (a text's characters, an
// id's form, where a field error or a keyboard may go): it has the last word.

declare namespace JSX {
  /** An element of a screen: one of the kit's components. */
  interface Element {
    readonly "nowayos:kit": true;
  }
  /** None: a screen is built from the kit's components only. */
  interface IntrinsicElements {}
  interface ElementChildrenAttribute {
    children: {};
  }
}

declare module "nowayos:kit" {
  /** One of the theme's colours, by name: never a colour value. */
  export type Colour = "background" | "text" | "text-muted" | "placeholder" | "button" | "button-text" | "input" | "input-border" | "focus" | "error";

  /** One of the theme's spacing tokens, which grow with the large-text theme. */
  export interface Space {
    readonly "nowayos:space": "xs" | "sm" | "md" | "lg" | "xl";
  }

  /** The theme's spacing tokens: `space.xs` to `space.xl`. */
  export const space: { readonly xs: Space; readonly sm: Space; readonly md: Space; readonly lg: Space; readonly xl: Space };

  /**
   * What a column or a row holds: elements, and the parts the application shows or hides
   * (`{props.ready && <X />}` is the prop's falsy value while it is hidden).
   */
  export type Child = JSX.Element | false | null | undefined | "" | 0;

  /** The props of `<Screen>`. */
  export interface ScreenProps {
    /** Exactly one element. */
    children: JSX.Element;
  }
  /** The root of every screen: exactly one child element. */
  export function Screen(props: ScreenProps): JSX.Element;

  /** What `style` on `<Column>` may name. */
  export interface ColumnStyle {
    /** A whole number of layout units (`16`, or `"16px"`), a design token or a spacing token (`space.md`). */
    gap?: number | `${number}px` | Space;
    /** A whole number of layout units (`16`, or `"16px"`), a design token or a spacing token (`space.md`). */
    padding?: number | `${number}px` | Space;
    /** A whole number of layout units (`16`, or `"16px"`) or a design token. */
    width?: number | `${number}px`;
    /** A whole number of layout units (`16`, or `"16px"`) or a design token. */
    height?: number | `${number}px`;
    /** A whole number of layout units (`16`, or `"16px"`) or a design token. */
    minWidth?: number | `${number}px`;
    /** A whole number of layout units (`16`, or `"16px"`) or a design token. */
    maxWidth?: number | `${number}px`;
    alignItems?: "flex-start" | "start" | "center" | "flex-end" | "end" | "stretch";
    justifyContent?: "flex-start" | "start" | "center" | "flex-end" | "end" | "space-between";
    /** One of the theme's colours, by name or as `var(--name)`. */
    backgroundColor?: Colour | `var(--${Colour})`;
    /** One of the theme's colours, by name or as `var(--name)`. */
    borderColor?: Colour | `var(--${Colour})`;
    /** A whole number from 0 to 16. */
    borderWidth?: 0 | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 | 10 | 11 | 12 | 13 | 14 | 15 | 16;
    /** A whole number from 0 to 64. */
    borderRadius?: number;
  }

  /** The props of `<Column>`. */
  export interface ColumnProps {
    /** An id: letters, digits, `-` and `_`, starting with a letter. */
    id?: string;
    /** A whole number of layout units (`16`, or `"16px"`), a design token or a spacing token (`space.md`). */
    gap?: number | `${number}px` | Space;
    /** A whole number of layout units (`16`, or `"16px"`), a design token or a spacing token (`space.md`). */
    padding?: number | `${number}px` | Space;
    style?: ColumnStyle;
    children?: Child | readonly Child[];
  }
  /** Children stacked vertically. */
  export function Column(props: ColumnProps): JSX.Element;

  /** What `style` on `<Row>` may name. */
  export interface RowStyle {
    /** A whole number of layout units (`16`, or `"16px"`), a design token or a spacing token (`space.md`). */
    gap?: number | `${number}px` | Space;
    /** A whole number of layout units (`16`, or `"16px"`), a design token or a spacing token (`space.md`). */
    padding?: number | `${number}px` | Space;
    /** A whole number of layout units (`16`, or `"16px"`) or a design token. */
    width?: number | `${number}px`;
    /** A whole number of layout units (`16`, or `"16px"`) or a design token. */
    height?: number | `${number}px`;
    /** A whole number of layout units (`16`, or `"16px"`) or a design token. */
    minWidth?: number | `${number}px`;
    /** A whole number of layout units (`16`, or `"16px"`) or a design token. */
    maxWidth?: number | `${number}px`;
    alignItems?: "flex-start" | "start" | "center" | "flex-end" | "end" | "stretch";
    justifyContent?: "flex-start" | "start" | "center" | "flex-end" | "end" | "space-between";
    /** One of the theme's colours, by name or as `var(--name)`. */
    backgroundColor?: Colour | `var(--${Colour})`;
    /** One of the theme's colours, by name or as `var(--name)`. */
    borderColor?: Colour | `var(--${Colour})`;
    /** A whole number from 0 to 16. */
    borderWidth?: 0 | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 | 10 | 11 | 12 | 13 | 14 | 15 | 16;
    /** A whole number from 0 to 64. */
    borderRadius?: number;
  }

  /** The props of `<Row>`. */
  export interface RowProps {
    /** An id: letters, digits, `-` and `_`, starting with a letter. */
    id?: string;
    /** A whole number of layout units (`16`, or `"16px"`), a design token or a spacing token (`space.md`). */
    gap?: number | `${number}px` | Space;
    /** A whole number of layout units (`16`, or `"16px"`), a design token or a spacing token (`space.md`). */
    padding?: number | `${number}px` | Space;
    style?: RowStyle;
    children?: Child | readonly Child[];
  }
  /** Children side by side. */
  export function Row(props: RowProps): JSX.Element;

  /** What `style` on `<Text>` may name. */
  export interface TextStyle {
    /** One of the theme's colours, by name or as `var(--name)`. */
    color?: Colour | `var(--${Colour})`;
    textAlign?: "left" | "start" | "center" | "right" | "end";
  }

  /** The props of `<Text>`. */
  export interface TextProps {
    /** An id: letters, digits, `-` and `_`, starting with a letter. */
    id?: string;
    role?: "display" | "title" | "heading" | "body" | "label" | "caption";
    weight?: "regular" | "bold";
    /** A whole number from 1 to 8. */
    lines?: 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8;
    align?: "start" | "center" | "end";
    style?: TextStyle;
    /** Literal text, or one prop the application sets. */
    children: string;
  }
  /** Text: literal, or one prop the application sets. */
  export function Text(props: TextProps): JSX.Element;

  /** What `style` on `<Button>` may name. */
  export interface ButtonStyle {
    /** One of the theme's colours, by name or as `var(--name)`. */
    backgroundColor?: Colour | `var(--${Colour})`;
    /** One of the theme's colours, by name or as `var(--name)`. */
    color?: Colour | `var(--${Colour})`;
  }

  /** The props of `<Button>`. */
  export type ButtonProps = ButtonOneOf & {
    /** An id: letters, digits, `-` and `_`, starting with a letter. */
    id?: string;
    variant?: "primary" | "secondary" | "quiet";
    style?: ButtonStyle;
    /** Its label: literal text. */
    children: string;
  };
  /** `<Button>` takes one of `onPress` and `onClick`, never both. */
  export type ButtonOneOf =
    | {
        /** A callback prop of the screen, which becomes an action; or a function that only calls one: `() => props.onSave()`. */
        onPress: () => void;
        onClick?: never;
      }
    | {
        /** A callback prop of the screen, which becomes an action; or a function that only calls one: `() => props.onSave()`. */
        onClick: () => void;
        onPress?: never;
      };
  /** A button that emits an action. */
  export function Button(props: ButtonProps): JSX.Element;

  /** The props of `<TextInput>`. */
  export interface TextInputProps {
    /** An id: letters, digits, `-` and `_`, starting with a letter. */
    id: string;
    /** A string literal. */
    placeholder?: string;
    /** A whole number from 1 to 64, or a design token. */
    maxLength?: number;
    inputMode?: "text" | "numeric" | "tel" | "email";
    children?: never;
  }
  /** A single-line text field. */
  export function TextInput(props: TextInputProps): JSX.Element;

  /** The props of `<Spacer>`. */
  export interface SpacerProps {
    /** An id: letters, digits, `-` and `_`, starting with a letter. */
    id?: string;
    /** A whole number of layout units (`16`, or `"16px"`), a design token or a spacing token (`space.md`). */
    size?: number | `${number}px` | Space;
    children?: never;
  }
  /** Empty space: flexible, or of a fixed size. */
  export function Spacer(props: SpacerProps): JSX.Element;

  /** The props of `<Checkbox>`. */
  export interface CheckboxProps {
    /** An id: letters, digits, `-` and `_`, starting with a letter. */
    id: string;
    /** A string literal. */
    label: string;
    children?: never;
  }
  /** A box the person ticks. */
  export function Checkbox(props: CheckboxProps): JSX.Element;

  /** The props of `<RadioGroup>`. */
  export interface RadioGroupProps {
    /** An id: letters, digits, `-` and `_`, starting with a letter. */
    id: string;
    /** 1 to 8 string literals. */
    options: readonly string[];
    children?: never;
  }
  /** One choice of a few options. */
  export function RadioGroup(props: RadioGroupProps): JSX.Element;

  /** What `style` on `<Card>` may name. */
  export interface CardStyle {
    /** A whole number of layout units (`16`, or `"16px"`), a design token or a spacing token (`space.md`). */
    gap?: number | `${number}px` | Space;
    /** A whole number of layout units (`16`, or `"16px"`), a design token or a spacing token (`space.md`). */
    padding?: number | `${number}px` | Space;
    /** A whole number of layout units (`16`, or `"16px"`) or a design token. */
    width?: number | `${number}px`;
    /** A whole number of layout units (`16`, or `"16px"`) or a design token. */
    height?: number | `${number}px`;
    /** A whole number of layout units (`16`, or `"16px"`) or a design token. */
    minWidth?: number | `${number}px`;
    /** A whole number of layout units (`16`, or `"16px"`) or a design token. */
    maxWidth?: number | `${number}px`;
    alignItems?: "flex-start" | "start" | "center" | "flex-end" | "end" | "stretch";
    justifyContent?: "flex-start" | "start" | "center" | "flex-end" | "end" | "space-between";
    /** One of the theme's colours, by name or as `var(--name)`. */
    backgroundColor?: Colour | `var(--${Colour})`;
    /** One of the theme's colours, by name or as `var(--name)`. */
    borderColor?: Colour | `var(--${Colour})`;
    /** A whole number from 0 to 16. */
    borderWidth?: 0 | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 | 10 | 11 | 12 | 13 | 14 | 15 | 16;
    /** A whole number from 0 to 64. */
    borderRadius?: number;
  }

  /** The props of `<Card>`. */
  export interface CardProps {
    /** An id: letters, digits, `-` and `_`, starting with a letter. */
    id?: string;
    /** A whole number of layout units (`16`, or `"16px"`), a design token or a spacing token (`space.md`). */
    gap?: number | `${number}px` | Space;
    /** A whole number of layout units (`16`, or `"16px"`), a design token or a spacing token (`space.md`). */
    padding?: number | `${number}px` | Space;
    style?: CardStyle;
    children?: Child | readonly Child[];
  }
  /** Children stacked vertically on a card. */
  export function Card(props: CardProps): JSX.Element;

  /** What `style` on `<Notice>` may name. */
  export interface NoticeStyle {
    /** A whole number of layout units (`16`, or `"16px"`), a design token or a spacing token (`space.md`). */
    gap?: number | `${number}px` | Space;
    /** A whole number of layout units (`16`, or `"16px"`), a design token or a spacing token (`space.md`). */
    padding?: number | `${number}px` | Space;
    /** A whole number of layout units (`16`, or `"16px"`) or a design token. */
    width?: number | `${number}px`;
    /** A whole number of layout units (`16`, or `"16px"`) or a design token. */
    height?: number | `${number}px`;
    /** A whole number of layout units (`16`, or `"16px"`) or a design token. */
    minWidth?: number | `${number}px`;
    /** A whole number of layout units (`16`, or `"16px"`) or a design token. */
    maxWidth?: number | `${number}px`;
    alignItems?: "flex-start" | "start" | "center" | "flex-end" | "end" | "stretch";
    justifyContent?: "flex-start" | "start" | "center" | "flex-end" | "end" | "space-between";
    /** One of the theme's colours, by name or as `var(--name)`. */
    backgroundColor?: Colour | `var(--${Colour})`;
    /** One of the theme's colours, by name or as `var(--name)`. */
    borderColor?: Colour | `var(--${Colour})`;
    /** A whole number from 0 to 16. */
    borderWidth?: 0 | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 | 10 | 11 | 12 | 13 | 14 | 15 | 16;
    /** A whole number from 0 to 64. */
    borderRadius?: number;
  }

  /** The props of `<Notice>`. */
  export interface NoticeProps {
    /** An id: letters, digits, `-` and `_`, starting with a letter. */
    id?: string;
    /** A whole number of layout units (`16`, or `"16px"`), a design token or a spacing token (`space.md`). */
    gap?: number | `${number}px` | Space;
    /** A whole number of layout units (`16`, or `"16px"`), a design token or a spacing token (`space.md`). */
    padding?: number | `${number}px` | Space;
    tone: "success" | "caution" | "warning";
    style?: NoticeStyle;
    children?: Child | readonly Child[];
  }
  /** Children stacked vertically on a notice, in a tone. */
  export function Notice(props: NoticeProps): JSX.Element;

  /** What `style` on `<HeaderBar>` may name. */
  export interface HeaderBarStyle {
    /** A whole number of layout units (`16`, or `"16px"`), a design token or a spacing token (`space.md`). */
    gap?: number | `${number}px` | Space;
    /** A whole number of layout units (`16`, or `"16px"`), a design token or a spacing token (`space.md`). */
    padding?: number | `${number}px` | Space;
    /** A whole number of layout units (`16`, or `"16px"`) or a design token. */
    width?: number | `${number}px`;
    /** A whole number of layout units (`16`, or `"16px"`) or a design token. */
    height?: number | `${number}px`;
    /** A whole number of layout units (`16`, or `"16px"`) or a design token. */
    minWidth?: number | `${number}px`;
    /** A whole number of layout units (`16`, or `"16px"`) or a design token. */
    maxWidth?: number | `${number}px`;
    alignItems?: "flex-start" | "start" | "center" | "flex-end" | "end" | "stretch";
    justifyContent?: "flex-start" | "start" | "center" | "flex-end" | "end" | "space-between";
    /** One of the theme's colours, by name or as `var(--name)`. */
    backgroundColor?: Colour | `var(--${Colour})`;
    /** One of the theme's colours, by name or as `var(--name)`. */
    borderColor?: Colour | `var(--${Colour})`;
    /** A whole number from 0 to 16. */
    borderWidth?: 0 | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 | 10 | 11 | 12 | 13 | 14 | 15 | 16;
    /** A whole number from 0 to 64. */
    borderRadius?: number;
  }

  /** The props of `<HeaderBar>`. */
  export interface HeaderBarProps {
    /** An id: letters, digits, `-` and `_`, starting with a letter. */
    id?: string;
    /** A whole number of layout units (`16`, or `"16px"`), a design token or a spacing token (`space.md`). */
    gap?: number | `${number}px` | Space;
    /** A whole number of layout units (`16`, or `"16px"`), a design token or a spacing token (`space.md`). */
    padding?: number | `${number}px` | Space;
    style?: HeaderBarStyle;
    children?: Child | readonly Child[];
  }
  /** Children side by side on a header bar. */
  export function HeaderBar(props: HeaderBarProps): JSX.Element;

  /** The props of `<ProgressBar>`. */
  export interface ProgressBarProps {
    /** An id: letters, digits, `-` and `_`, starting with a letter. */
    id?: string;
    /** A prop of the screen, whose value the application sets. */
    value: number;
    children?: never;
  }
  /** A bar the application fills, from a prop. */
  export function ProgressBar(props: ProgressBarProps): JSX.Element;

  /** The props of `<Icon>`. */
  export interface IconProps {
    /** An id: letters, digits, `-` and `_`, starting with a letter. */
    id?: string;
    name: "clipboard-list" | "fuel" | "house" | "message-square" | "shield-check" | "user-check";
    children?: never;
  }
  /** One of Lucide's icons, by name. */
  export function Icon(props: IconProps): JSX.Element;

  /** The props of `<ScrollView>`. */
  export interface ScrollViewProps {
    /** An id: letters, digits, `-` and `_`, starting with a letter. */
    id?: string;
    /** Exactly one element. */
    children: JSX.Element;
  }
  /** One child shown through a view that scrolls: at most one in a screen, never nested. */
  export function ScrollView(props: ScrollViewProps): JSX.Element;

  /** The props of `<Select>`. */
  export interface SelectProps {
    /** An id: letters, digits, `-` and `_`, starting with a letter. */
    id: string;
    /** A string literal. */
    placeholder?: string;
    /** 1 to 64 string literals. */
    options: readonly string[];
    /** A whole number from 3 to 8. */
    rows?: 3 | 4 | 5 | 6 | 7 | 8;
    children?: never;
  }
  /** One choice of up to 64 options in a list that scrolls inside itself. */
  export function Select(props: SelectProps): JSX.Element;

  /** The props of `<Keyboard>`. */
  export interface KeyboardProps {
    /** An id: letters, digits, `-` and `_`, starting with a letter. */
    id?: string;
    children?: never;
  }
  /** The on-screen keyboard: at most one in a screen, and only in one with a text input. */
  export function Keyboard(props: KeyboardProps): JSX.Element;

  /** The props of `<FieldError>`. */
  export interface FieldErrorProps {
    /** An id: letters, digits, `-` and `_`, starting with a letter. */
    id?: string;
    /** The id of a field on the same screen. */
    for: string;
    children?: never;
  }
  /** Where the errors the application sets for a field show. */
  export function FieldError(props: FieldErrorProps): JSX.Element;
}
