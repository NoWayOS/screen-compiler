// Synthetic Screen Compiler fixture: scrolling everywhere (contract v17).
// A side panel that scrolls down beside a main area holding a row of
// tiles that scrolls across, and a grid that scrolls both ways - each a
// container whose style names overflow. Written for Screen Compiler's
// tests; not taken from any product.
import { Button, Card, Column, Row, Screen, Text } from "nowayos:kit";

const GAP = 8;
const EDGE = 12;

type PanelsProps = {
  onPick: () => void;
};

export function Panels(props: PanelsProps) {
  return (
    <Screen>
      <Row gap={GAP} padding={EDGE}>
        <Column gap={GAP} style={{ width: 200, overflowY: "auto" }}>
          <Text role="heading">Rooms</Text>
          <Button variant="secondary" onPress={props.onPick}>Atrium</Button>
          <Button variant="secondary" onPress={props.onPick}>Boathouse</Button>
          <Button variant="secondary" onPress={props.onPick}>Courtyard</Button>
          <Button variant="secondary" onPress={props.onPick}>Dairy</Button>
          <Button variant="secondary" onPress={props.onPick}>East wing</Button>
          <Button variant="secondary" onPress={props.onPick}>Folly</Button>
          <Button variant="secondary" onPress={props.onPick}>Gatehouse</Button>
          <Button variant="secondary" onPress={props.onPick}>Hall</Button>
        </Column>
        <Column gap={GAP} style={{ width: 560 }}>
          <Row gap={GAP} style={{ overflowX: "scroll" }}>
            <Card padding={EDGE}><Text>Monday</Text></Card>
            <Card padding={EDGE}><Text>Tuesday</Text></Card>
            <Card padding={EDGE}><Text>Wednesday</Text></Card>
            <Card padding={EDGE}><Text>Thursday</Text></Card>
            <Card padding={EDGE}><Text>Friday</Text></Card>
            <Card padding={EDGE}><Text>Saturday</Text></Card>
            <Card padding={EDGE}><Text>Sunday</Text></Card>
          </Row>
          <Column gap={GAP} style={{ overflow: "auto", height: 160 }}>
            <Row gap={GAP}>
              <Card padding={EDGE} style={{ width: 200 }}><Text>Morning tours</Text></Card>
              <Card padding={EDGE} style={{ width: 200 }}><Text>Afternoon tours</Text></Card>
              <Card padding={EDGE} style={{ width: 200 }}><Text>Evening tours</Text></Card>
            </Row>
            <Row gap={GAP}>
              <Card padding={EDGE} style={{ width: 200 }}><Text>Gardens</Text></Card>
              <Card padding={EDGE} style={{ width: 200 }}><Text>Kitchens</Text></Card>
              <Card padding={EDGE} style={{ width: 200 }}><Text>Cellars</Text></Card>
            </Row>
            <Row gap={GAP}>
              <Card padding={EDGE} style={{ width: 200 }}><Text>Library</Text></Card>
              <Card padding={EDGE} style={{ width: 200 }}><Text>Long gallery</Text></Card>
              <Card padding={EDGE} style={{ width: 200 }}><Text>Roof walk</Text></Card>
            </Row>
            <Row gap={GAP}>
              <Card padding={EDGE} style={{ width: 200 }}><Text>Stables</Text></Card>
              <Card padding={EDGE} style={{ width: 200 }}><Text>Orchard</Text></Card>
              <Card padding={EDGE} style={{ width: 200 }}><Text>Lake shore</Text></Card>
            </Row>
          </Column>
        </Column>
      </Row>
    </Screen>
  );
}
