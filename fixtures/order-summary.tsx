// Synthetic Screen Compiler fixture: lists. A café's order summary: the
// steps along the top from a const array, expanded at build time, and the
// order's lines from a prop - at most four, sliced - as rows the
// application shows and fills. Written for Screen Compiler's tests; not
// taken from any product.
import { Button, Card, Column, Row, Screen, Spacer, Text } from "nowayos:kit";

const GAP = 8;
const EDGE = 16;
const STEPS = ["1 Choose", "2 Check", "3 Pay"];

type Line = { name: string; price: string };

type OrderSummaryProps = {
  lines: Line[];
  total: string;
  onPay: () => void;
};

export function OrderSummary(props: OrderSummaryProps) {
  return (
    <Screen>
      <Column gap={GAP} padding={EDGE}>
        <Row gap={EDGE}>
          {STEPS.map((step) => (
            <Text key={step} role="label">
              {step}
            </Text>
          ))}
        </Row>
        <Text role="title">Your order</Text>
        <Card gap={GAP} padding={EDGE}>
          {props.lines.slice(0, 4).map((line) => (
            <Row key={line.name} gap={GAP}>
              <Text>{line.name}</Text>
              <Spacer />
              <Text>{line.price}</Text>
            </Row>
          ))}
        </Card>
        <Row gap={GAP}>
          <Text role="heading">Total</Text>
          <Spacer />
          <Text role="heading">{props.total}</Text>
        </Row>
        <Button onPress={props.onPay}>Pay</Button>
      </Column>
    </Screen>
  );
}
