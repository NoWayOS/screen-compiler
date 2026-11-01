// Synthetic Screen Compiler fixture: live figures and a progress bar. The
// application counts the amount and the litres up, each in a slot, and
// fills the bar as it goes. Written for Screen Compiler's tests; not taken from
// any product.
import { Button, Card, Column, ProgressBar, Row, Screen, Spacer, Text } from "nowayos:kit";

const GAP = 12;
const EDGE = 16;

type FuellingProps = {
  amount: string;
  litres: string;
  fuelled: number;
  onDone: () => void;
};

export function Fuelling(props: FuellingProps) {
  return (
    <Screen>
      <Column gap={GAP} padding={EDGE}>
        <Text role="title">Fuelling</Text>
        <Card gap={GAP} padding={EDGE}>
          <Text role="label">Amount</Text>
          <Text role="display">{props.amount}</Text>
          <Row gap={GAP}>
            <Text>Litres</Text>
            <Text>{props.litres}</Text>
          </Row>
          <ProgressBar value={props.fuelled} />
        </Card>
        <Spacer />
        <Row gap={GAP}>
          <Spacer />
          <Button onPress={props.onDone}>Done</Button>
        </Row>
      </Column>
    </Screen>
  );
}
