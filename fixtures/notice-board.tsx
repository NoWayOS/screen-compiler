// Synthetic Screen Compiler fixture: text set on several lines. A title,
// a notice whose text wraps onto up to three lines, a card whose text from
// the application wraps onto up to four, and a row of two buttons. Written
// for Screen Compiler's tests; not taken from any product.
import { Button, Card, Column, Notice, Row, Screen, Spacer, Text } from "nowayos:kit";

const GAP = 12;
const EDGE = 16;

type NoticeBoardProps = {
  update: string;
  onBack: () => void;
  onDone: () => void;
};

export function NoticeBoard(props: NoticeBoardProps) {
  return (
    <Screen>
      <Column gap={GAP} padding={EDGE}>
        <Text role="title">Before you start</Text>
        <Notice tone="caution" padding={EDGE}>
          <Text lines={3}>
            {"This screen forgets you when you finish, and nobody who uses it after you can see anything you typed or chose on it."}
          </Text>
        </Notice>
        <Card gap={GAP} padding={EDGE}>
          <Text role="heading">Today</Text>
          <Text lines={4}>{props.update}</Text>
        </Card>
        <Spacer />
        <Row gap={GAP}>
          <Button variant="secondary" onPress={props.onBack}>
            Back
          </Button>
          <Spacer />
          <Button onPress={props.onDone}>Done</Button>
        </Row>
      </Column>
    </Screen>
  );
}
