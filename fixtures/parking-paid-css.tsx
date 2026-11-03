// Synthetic Screen Compiler fixture: `parking-paid.tsx` written as CSS
// writes it - every length a string in px and every colour the theme's
// custom property, `var(--name)` - which must convert and draw exactly as
// that screen does (its approved images are copies of parking-paid's).
// Written for Screen Compiler's tests; not taken from any product.
import { Button, Card, Column, Row, Screen, Text, space } from "nowayos:kit";

type ParkingPaidProps = {
  until: string;
  onReceipt: () => void;
  onDone: () => void;
};

export function ParkingPaid(props: ParkingPaidProps) {
  return (
    <Screen>
      <Column gap={space.md} padding="24px" style={{ alignItems: "center" }}>
        <Text role="title" style={{ textAlign: "center" }}>
          Parking paid
        </Text>
        <Card
          style={{
            width: "360px",
            gap: "4px",
            padding: space.md,
            backgroundColor: "var(--input)",
            borderColor: "var(--input-border)",
            borderWidth: 2,
            borderRadius: 12,
          }}
        >
          <Text role="label" style={{ color: "var(--text-muted)" }}>
            Valid until
          </Text>
          <Text role="heading" style={{ textAlign: "right" }}>
            {props.until}
          </Text>
        </Card>
        <Row
          gap={space.sm}
          style={{ minWidth: "360px", maxWidth: "480px", height: "64px", justifyContent: "space-between" }}
        >
          <Button variant="secondary" onPress={props.onReceipt}>
            Receipt
          </Button>
          <Button onPress={props.onDone} style={{ backgroundColor: "var(--text)", color: "var(--background)" }}>
            Done
          </Button>
        </Row>
        <Text role="caption" style={{ color: "var(--error)" }}>
          Leave within 15 minutes of paying.
        </Text>
      </Column>
    </Screen>
  );
}
