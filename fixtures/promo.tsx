// Synthetic Screen Compiler fixture: a generic welcome banner on a gradient, cards
// casting the theme's shadows, and a see-through note (contract v22).
// Written for Screen Compiler's tests; not taken from any product.
import { Card, Column, Notice, Row, Screen, Text } from "nowayos:kit";

const GAP = 24;
const EDGE = 32;

export default function Promo() {
  return (
    <Screen>
      <Column gap={GAP} padding={EDGE}>
        <Column padding={GAP} style={{ background: "linear-gradient(to right, button, text)", borderRadius: 16 }}>
          <Text role="title" style={{ color: "button-text" }}>
            A station with nothing else to do
          </Text>
        </Column>
        <Row gap={GAP}>
          <Card padding={16} style={{ boxShadow: "low", width: 200 }}>
            <Text>Resting</Text>
          </Card>
          <Card padding={16} style={{ boxShadow: "medium", width: 200 }}>
            <Text>Lifted</Text>
          </Card>
          <Card padding={16} style={{ boxShadow: "high", width: 200 }}>
            <Text>Over everything</Text>
          </Card>
        </Row>
        <Column padding={GAP} style={{ background: "radial-gradient(input, placeholder)", borderRadius: 16 }}>
          <Notice tone="caution" padding={12} style={{ opacity: 0.6 }}>
            <Text>A note drawn see-through</Text>
          </Notice>
        </Column>
      </Column>
    </Screen>
  );
}
