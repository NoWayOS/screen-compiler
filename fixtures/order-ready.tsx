// Synthetic Screen Compiler fixture: an order ready to collect, written for
// contract v7. Its spacing is the theme's - `space` from the kit, so
// the large-text theme spaces it half again - with one gap of the screen's
// own to show a number stays fixed; its title, message and thanks are
// centred, and the collection point set against the end of its card.
// Written for Screen Compiler's tests; not taken from any product.
import { Button, Card, Column, Screen, Spacer, Text, space } from "nowayos:kit";

const TIGHT = 4;

type OrderReadyProps = {
  note: string;
  onDone: () => void;
};

export function OrderReady(props: OrderReadyProps) {
  return (
    <Screen>
      <Column gap={space.sm} padding={space.md}>
        <Text role="title" align="center">
          Your order is ready
        </Text>
        <Text align="center" lines={2}>
          Collect it from the counter shown below, with the number on your receipt.
        </Text>
        <Card gap={TIGHT} padding={space.md}>
          <Text role="label">Collection point</Text>
          <Text role="heading" align="end">
            Counter 4
          </Text>
        </Card>
        <Spacer size={space.lg} />
        <Text role="caption" align="center">
          {props.note}
        </Text>
        <Button onPress={props.onDone}>Done</Button>
      </Column>
    </Screen>
  );
}
