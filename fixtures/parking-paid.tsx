// Synthetic Screen Compiler fixture: a car park's ticket paid, written for the
// declared style subset. Every property a style may name is here,
// each a theme colour by its name or a number of layout units: a centred
// column, a card with a fill, border and corners of its own, a row of
// buttons spread apart within a least and greatest width, a muted label, an
// end-aligned time, and a button in the theme's text and background
// colours. Written for Screen Compiler's tests; not taken from any product.
import { Button, Card, Column, Row, Screen, Text, space } from "nowayos:kit";

const EDGE = 24;

type ParkingPaidProps = {
  until: string;
  onReceipt: () => void;
  onDone: () => void;
};

export function ParkingPaid(props: ParkingPaidProps) {
  return (
    <Screen>
      <Column gap={space.md} padding={EDGE} style={{ alignItems: "center" }}>
        <Text role="title" style={{ textAlign: "center" }}>
          Parking paid
        </Text>
        <Card
          style={{
            width: 360,
            gap: 4,
            padding: space.md,
            backgroundColor: "input",
            borderColor: "input-border",
            borderWidth: 2,
            borderRadius: 12,
          }}
        >
          <Text role="label" style={{ color: "text-muted" }}>
            Valid until
          </Text>
          <Text role="heading" style={{ textAlign: "right" }}>
            {props.until}
          </Text>
        </Card>
        <Row
          gap={space.sm}
          style={{ minWidth: 360, maxWidth: 480, height: 64, justifyContent: "space-between" }}
        >
          <Button variant="secondary" onPress={props.onReceipt}>
            Receipt
          </Button>
          <Button onPress={props.onDone} style={{ backgroundColor: "text", color: "background" }}>
            Done
          </Button>
        </Row>
        <Text role="caption" style={{ color: "error" }}>
          Leave within 15 minutes of paying.
        </Text>
      </Column>
    </Screen>
  );
}
