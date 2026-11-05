// Synthetic Screen Compiler fixture: a parcel locker's status screen, written for
// subset v1's conditional rendering and text from props. What the
// application knows - the locker, the time to collect by, whether the
// parcel is ready or late - reaches the screen as props: a prop alone in a
// <Text> is a slot the application fills, and each condition a group it
// shows or hides. The locker's number can be selected and copied (contract
// v24). Written for Screen Compiler's tests; not taken from any product.
import { Button, Column, Row, Screen, Spacer, Text } from "nowayos:kit";

const GAP = 12;
const EDGE = 24;

type ParcelStatusProps = {
  locker: string;
  collectBy: string;
  ready: boolean;
  late: boolean;
  lateNotice: string;
  onOpen: () => void;
  onHelp: () => void;
};

export function ParcelStatus(props: ParcelStatusProps) {
  return (
    <Screen>
      <Column gap={GAP} padding={EDGE}>
        <Text id="heading">Your parcel</Text>
        {props.ready ? (
          <Text>It is ready to collect.</Text>
        ) : (
          <Text>It is on its way to this locker.</Text>
        )}
        <Row gap={GAP}>
          <Text>Locker:</Text>
          <Text id="locker" style={{ userSelect: "text" }}>
            {props.locker}
          </Text>
        </Row>
        <Row gap={GAP}>
          <Text>Collect by:</Text>
          <Text>{props.collectBy}</Text>
        </Row>
        {props.late && <Text>{props.lateNotice}</Text>}
        <Spacer />
        <Row gap={GAP}>
          <Button onPress={props.onHelp}>Help</Button>
          <Spacer />
          {props.ready && (
            <Button id="open" onPress={props.onOpen}>
              Open the locker
            </Button>
          )}
        </Row>
      </Column>
    </Screen>
  );
}
