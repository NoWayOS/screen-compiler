// Synthetic Screen Compiler fixture: a virtual list (contract v18). An inbox
// of as many messages as the application counts, shown through a pool of
// twelve rows, each the sender and the subject; rebound as it scrolls.
// Written for Screen Compiler's tests; not taken from any product.
import { Button, Column, HeaderBar, Row, Screen, Text, VirtualList } from "nowayos:kit";

const GAP = 8;
const EDGE = 12;
const ROW = 48;

type Message = {
  from: string;
  subject: string;
};

type InboxProps = {
  messages: Message[];
  onCompose: () => void;
};

export function Inbox(props: InboxProps) {
  return (
    <Screen>
      <Column gap={GAP}>
        <HeaderBar gap={GAP} padding={EDGE}>
          <Text role="heading">Inbox</Text>
          <Button variant="secondary" onPress={props.onCompose}>Write</Button>
        </HeaderBar>
        <VirtualList rowHeight={ROW} style={{ height: 380 }}>
          {props.messages.slice(0, 12).map((message) => (
            <Row gap={GAP} padding={EDGE}>
              <Text weight="bold">{message.from}</Text>
              <Text>{message.subject}</Text>
            </Row>
          ))}
        </VirtualList>
      </Column>
    </Screen>
  );
}
