// Synthetic Screen Compiler fixture: a restaurant pager's screen, written
// for contract v14's text that mixes literals and a prop. Each <Text> holds
// literal words and one prop - in JSX, or as a template literal - and the
// words are compiled in while the application sets only the prop's part: a
// greeting, a count, a sentence that wraps, and a table number in the
// display role. Written for Screen Compiler's tests; not taken from any
// product.
import { Button, Column, Screen, Spacer, Text } from "nowayos:kit";

const GAP = 12;
const EDGE = 24;

type TableReadyProps = {
  name: string;
  count: string;
  place: string;
  table: string;
  onDone: () => void;
};

export function TableReady(props: TableReadyProps) {
  return (
    <Screen>
      <Column gap={GAP} padding={EDGE}>
        <Text role="heading">Hello, {props.name}!</Text>
        <Text>{props.count} guests on this booking</Text>
        <Text lines={2}>
          {`Your table by the ${props.place} is ready. Follow the green line.`}
        </Text>
        <Text role="display">TABLE {props.table}</Text>
        <Spacer />
        <Button onPress={props.onDone}>Done</Button>
      </Column>
    </Screen>
  );
}
