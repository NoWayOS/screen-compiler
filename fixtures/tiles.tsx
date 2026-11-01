// Synthetic Screen Compiler fixture: icons. A header bar with an icon and
// a title, and two tiles on cards, each with an icon, a name and a button.
// The icons take the colour of text where they stand. Written for
// Screen Compiler's tests; not taken from any product.
import { Button, Card, Column, HeaderBar, Icon, Row, Screen, Text } from "nowayos:kit";

const GAP = 12;
const EDGE = 16;

type TilesProps = {
  onFuel: () => void;
  onVisit: () => void;
};

export function Tiles(props: TilesProps) {
  return (
    <Screen>
      <Column gap={GAP}>
        <HeaderBar gap={GAP} padding={EDGE}>
          <Icon name="house" />
          <Text role="heading">Choose a service</Text>
        </HeaderBar>
        <Row gap={GAP} padding={EDGE}>
          <Card gap={GAP} padding={EDGE}>
            <Icon name="fuel" />
            <Text>Fuel</Text>
            <Button onPress={props.onFuel}>Start</Button>
          </Card>
          <Card gap={GAP} padding={EDGE}>
            <Icon name="user-check" />
            <Text>Visitors</Text>
            <Button variant="secondary" onPress={props.onVisit}>
              Sign in
            </Button>
          </Card>
        </Row>
      </Column>
    </Screen>
  );
}
